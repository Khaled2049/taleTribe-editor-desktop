//! Publishing a local story to the website.
//!
//! One direction only: this computer is the source of truth and the website
//! receives it. A publish first reads the website's copy and stops, before
//! writing anything, if something there changed since this app last saw it;
//! the writer then chooses per item whether to overwrite it or leave it.
//!
//! Every successful write is recorded in the local database straight away, so
//! a publish that is interrupted can simply be run again.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::plan::{self, asset_refs, with_hosted_images};
use super::remote::{
    ChapterFields, EntityKind, Remote, RemoteChapter, RemoteEntity, RemoteStory, StoryFields,
};
use super::SyncError;
use crate::error::AppResult;

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Send the story but release nothing new to readers.
    Draft,
    /// Send the story and make it and its written chapters public.
    Publish,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Choice {
    Overwrite,
    Skip,
}

/// The writer's answers, by `Conflict::id`. `RECREATE` answers `Outcome::Gone`.
pub type Decisions = HashMap<String, Choice>;
pub const RECREATE: &str = "recreate";

/// Something changed on the website since this app last saw it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Conflict {
    pub id: String,
    /// "story", "chapter", "character" or "place".
    pub kind: &'static str,
    pub label: String,
    /// The local copy was deleted; overwriting deletes it on the website too.
    pub deleted_here: bool,
}

#[derive(Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub created: usize,
    pub updated: usize,
    pub deleted: usize,
    pub images: usize,
    pub released: usize,
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(
    tag = "status",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Outcome {
    Done {
        remote_id: String,
        public: bool,
        summary: Summary,
    },
    /// The writer has not accepted the current writer agreement on the website.
    NeedsAgreement,
    Blocked {
        problems: Vec<String>,
    },
    Conflicts {
        conflicts: Vec<Conflict>,
    },
    /// The website story this one was published as no longer exists.
    Gone,
}

struct Local {
    id: String,
    /// Chapter title or entity name.
    label: String,
    remote_id: Option<String>,
    remote_revision: Option<i64>,
    dirty: bool,
    /// Compared when clearing `dirty`, so an edit made mid-publish is not lost.
    updated_at: i64,
    /// Chapter HTML, or the entity's fields as story-data's input.
    body: Value,
}

struct StoryRow {
    title: String,
    description: String,
    remote_id: Option<String>,
    revision: Option<i64>,
    dirty: bool,
}

struct Tombstone {
    remote_id: String,
    remote_revision: i64,
    label: String,
}

const CHAPTERS: &str = "chapters";

fn load(conn: &Connection, table: &str, story_id: &str) -> AppResult<Vec<Local>> {
    let (label, body, order) = match table {
        CHAPTERS => ("title", "json_quote(content)", "position, created_at"),
        "characters" => (
            "name",
            "json_object('name', name, 'age', age, 'soul', soul, 'personality', personality,
                'voice', voice, 'backstory', backstory, 'affiliations', affiliations, 'notes', notes)",
            "created_at",
        ),
        _ => (
            "name",
            "json_object('name', name, 'description', description, 'atmosphere', atmosphere,
                'geography', geography, 'history', history, 'significance', significance, 'notes', notes)",
            "created_at",
        ),
    };
    let mut stmt = conn.prepare(&format!(
        "SELECT id, {label}, remote_id, remote_revision, dirty, updated_at, {body}
         FROM {table} WHERE story_id = ?1 ORDER BY {order}"
    ))?;
    let rows = stmt.query_map([story_id], |row| {
        let body: String = row.get(6)?;
        Ok(Local {
            id: row.get(0)?,
            label: row.get(1)?,
            remote_id: row.get(2)?,
            remote_revision: row.get(3)?,
            dirty: row.get(4)?,
            updated_at: row.get(5)?,
            body: serde_json::from_str(&body).unwrap_or(Value::Null),
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

fn tombstones(conn: &Connection, story_id: &str, kind: &str) -> AppResult<Vec<Tombstone>> {
    let mut stmt = conn.prepare(
        "SELECT remote_id, remote_revision, label FROM tombstones WHERE story_id = ?1 AND kind = ?2",
    )?;
    let rows = stmt.query_map([story_id, kind], |row| {
        Ok(Tombstone {
            remote_id: row.get(0)?,
            remote_revision: row.get(1)?,
            label: row.get(2)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Records that `local` now matches the website at `revision`.
fn record(
    conn: &Connection,
    table: &str,
    local: &Local,
    remote_id: &str,
    revision: i64,
) -> AppResult<()> {
    conn.execute(
        &format!(
            "UPDATE {table} SET remote_id = ?2, remote_revision = ?3,
                dirty = CASE WHEN updated_at = ?4 THEN 0 ELSE 1 END
             WHERE id = ?1"
        ),
        params![local.id, remote_id, revision, local.updated_at],
    )?;
    Ok(())
}

/// Records the website story's id and revision. When the local title and
/// description were sent, the story is clean unless they were edited meanwhile.
/// (A story's `updated_at` moves on every chapter save, so it cannot tell.)
fn record_story(
    conn: &Connection,
    story_id: &str,
    website: &RemoteStory,
    carried_edits: bool,
) -> AppResult<()> {
    conn.execute(
        "UPDATE stories SET remote_id = ?2, remote_revision = ?3,
            dirty = CASE WHEN ?4 AND title = ?5 AND description = ?6 THEN 0 ELSE dirty END
         WHERE id = ?1",
        params![
            story_id,
            website.id,
            website.revision,
            carried_edits,
            website.fields.title,
            website.fields.description
        ],
    )?;
    Ok(())
}

fn drop_tombstone(conn: &Connection, kind: &str, remote_id: &str) -> AppResult<()> {
    conn.execute(
        "DELETE FROM tombstones WHERE kind = ?1 AND remote_id = ?2",
        [kind, remote_id],
    )?;
    Ok(())
}

/// Forgets that a story was ever published, so the next publish makes a new one.
fn unlink(conn: &mut Connection, story_id: &str) -> AppResult<()> {
    let tx = conn.transaction()?;
    tx.execute(
        "DELETE FROM asset_uploads WHERE remote_story_id = (SELECT remote_id FROM stories WHERE id = ?1)",
        [story_id],
    )?;
    for table in [CHAPTERS, "characters", "places"] {
        tx.execute(
            &format!("UPDATE {table} SET remote_id = NULL, remote_revision = NULL, dirty = 1 WHERE story_id = ?1"),
            [story_id],
        )?;
    }
    tx.execute(
        "UPDATE stories SET remote_id = NULL, remote_revision = NULL, dirty = 1 WHERE id = ?1",
        [story_id],
    )?;
    tx.execute("DELETE FROM tombstones WHERE story_id = ?1", [story_id])?;
    Ok(tx.commit()?)
}

/// What story-data expects for a character or place: this app's fields, plus
/// the parts only the website edits, carried over from its current copy.
fn entity_input(kind: EntityKind, local: &Local, remote: Option<&RemoteEntity>) -> Value {
    let mut input = local.body.clone();
    let kept = |field: &str, empty: Value| {
        remote
            .map(|r| r.body[field].clone())
            .filter(|v| !v.is_null())
            .unwrap_or(empty)
    };
    match kind {
        EntityKind::Character => {
            input["artUrl"] = kept("artUrl", json!(""));
            input["relationships"] = kept("relationships", json!([]));
        }
        EntityKind::Place => input["imageUrl"] = kept("imageUrl", json!("")),
    }
    input
}

/// Positions for the chapters in their local order. A chapter keeps the
/// position it has on the website when that still sorts after its predecessor;
/// otherwise it gets a free position that does. No value is ever one another
/// chapter holds, so applying them in any order cannot collide.
fn positions(current: &[Option<f64>], mut taken: Vec<f64>) -> Vec<f64> {
    // story-data stores ten decimal places; six leaves halving room without rounding surprises.
    let round = |value: f64| (value * 1e6).round() / 1e6;
    let mut previous = -1.0_f64;
    current
        .iter()
        .map(|&now| {
            if let Some(position) = now.filter(|&p| p > previous) {
                previous = position;
                return position;
            }
            let ceiling = taken
                .iter()
                .copied()
                .filter(|&p| p > previous)
                .fold(f64::INFINITY, f64::min);
            let between = round(if ceiling.is_finite() {
                (previous + ceiling) / 2.0
            } else {
                previous + 1.0
            });
            let fits = between >= 0.0 && between > previous && between < ceiling;
            // No room left between neighbours: go past everything instead.
            let position = if fits {
                between
            } else {
                taken.iter().copied().fold(previous, f64::max).floor() + 1.0
            };
            taken.push(position);
            previous = position;
            position
        })
        .collect()
}

pub struct Publisher<'a, R: Remote> {
    pub db: &'a Mutex<Connection>,
    pub assets_dir: &'a Path,
    pub remote: &'a R,
    /// The signed-in account; a website story owned by anyone else is never written.
    pub uid: &'a str,
    pub progress: &'a (dyn Fn(String) + Send + Sync),
}

impl<R: Remote> Publisher<'_, R> {
    /// Runs `work` with the database. The lock is never held across a request.
    fn db<T>(&self, work: impl FnOnce(&mut Connection) -> AppResult<T>) -> AppResult<T> {
        work(
            &mut self
                .db
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
        )
    }

    pub async fn publish(
        &self,
        story_id: &str,
        mode: Mode,
        decisions: &Decisions,
    ) -> AppResult<Outcome> {
        if !self.remote.agreement_accepted().await? {
            return Ok(Outcome::NeedsAgreement);
        }
        let plan = self.db(|conn| plan::preview(conn, self.assets_dir, story_id))?;
        if !plan.problems.is_empty() {
            return Ok(Outcome::Blocked {
                problems: plan.problems,
            });
        }

        // ── Read the website's copy ──────────────────────────────────────────
        let StoryRow { title, description, mut remote_id, revision: story_revision, dirty: story_dirty } =
            self.db(|conn| {
                Ok(conn.query_row(
                    "SELECT title, description, remote_id, remote_revision, dirty FROM stories WHERE id = ?1",
                    [story_id],
                    |row| {
                        Ok(StoryRow {
                            title: row.get(0)?,
                            description: row.get(1)?,
                            remote_id: row.get(2)?,
                            revision: row.get(3)?,
                            dirty: row.get(4)?,
                        })
                    },
                )?)
            })?;

        let mut website: Option<RemoteStory> = None;
        if let Some(id) = &remote_id {
            match self.remote.get_story(id).await {
                Ok(story) if story.owner_id == self.uid => website = Some(story),
                Ok(_) | Err(SyncError::NotFound) => {
                    if decisions.get(RECREATE) != Some(&Choice::Overwrite) {
                        return Ok(Outcome::Gone);
                    }
                    self.db(|conn| unlink(conn, story_id))?;
                    remote_id = None;
                }
                Err(error) => return Err(error.into()),
            }
        }

        let mut chapters = self.db(|conn| load(conn, CHAPTERS, story_id))?;
        let mut remote_chapters: Vec<RemoteChapter> = Vec::new();
        let mut remote_entities: HashMap<EntityKind, Vec<RemoteEntity>> = HashMap::new();
        if let Some(id) = &remote_id {
            remote_chapters = self.remote.list_chapters(id).await?;
            for kind in [EntityKind::Character, EntityKind::Place] {
                remote_entities.insert(kind, self.remote.list_entities(id, kind).await?);
            }
        }

        // ── Stop for anything that changed on the website ────────────────────
        let mut conflicts = Vec::new();
        if let Some(story) = &website {
            if story_dirty && Some(story.revision) != story_revision {
                conflicts.push(Conflict {
                    id: "story".into(),
                    kind: "story",
                    label: "Title and description".into(),
                    deleted_here: false,
                });
            }
        }
        let chapter_graves = self.db(|conn| tombstones(conn, story_id, "chapter"))?;
        for chapter in &chapters {
            let theirs = remote_chapters
                .iter()
                .find(|r| Some(&r.id) == chapter.remote_id.as_ref());
            if theirs.is_some_and(|r| Some(r.revision) != chapter.remote_revision) {
                conflicts.push(Conflict {
                    id: format!("chapter:{}", chapter.id),
                    kind: "chapter",
                    label: chapter.label.clone(),
                    deleted_here: false,
                });
            }
        }
        for grave in &chapter_graves {
            if remote_chapters
                .iter()
                .any(|r| r.id == grave.remote_id && r.revision != grave.remote_revision)
            {
                conflicts.push(Conflict {
                    id: format!("deleted:chapter:{}", grave.remote_id),
                    kind: "chapter",
                    label: grave.label.clone(),
                    deleted_here: true,
                });
            }
        }
        let mut entities = HashMap::new();
        for kind in [EntityKind::Character, EntityKind::Place] {
            let locals = self.db(|conn| load(conn, kind.table(), story_id))?;
            let graves = self.db(|conn| tombstones(conn, story_id, kind.tombstone()))?;
            let theirs = remote_entities
                .get(&kind)
                .map(Vec::as_slice)
                .unwrap_or_default();
            for local in &locals {
                let remote = theirs
                    .iter()
                    .find(|r| Some(&r.id) == local.remote_id.as_ref());
                if local.dirty && remote.is_some_and(|r| Some(r.revision) != local.remote_revision)
                {
                    conflicts.push(Conflict {
                        id: format!("{}:{}", kind.tombstone(), local.id),
                        kind: kind.tombstone(),
                        label: local.label.clone(),
                        deleted_here: false,
                    });
                }
            }
            for grave in &graves {
                if theirs
                    .iter()
                    .any(|r| r.id == grave.remote_id && r.revision != grave.remote_revision)
                {
                    conflicts.push(Conflict {
                        id: format!("deleted:{}:{}", kind.tombstone(), grave.remote_id),
                        kind: kind.tombstone(),
                        label: grave.label.clone(),
                        deleted_here: true,
                    });
                }
            }
            entities.insert(kind, (locals, graves));
        }
        if conflicts
            .iter()
            .any(|conflict| !decisions.contains_key(&conflict.id))
        {
            return Ok(Outcome::Conflicts { conflicts });
        }
        let skipped = |id: &str| decisions.get(id) == Some(&Choice::Skip);
        let mut summary = Summary::default();

        // ── Story ────────────────────────────────────────────────────────────
        let mut website = match website {
            Some(story) => story,
            None => {
                (self.progress)("Creating the story on the website".into());
                let fields = StoryFields {
                    title: title.clone(),
                    description: description.clone(),
                    ..StoryFields::default()
                };
                let created = self.remote.create_story(&fields).await?;
                self.db(|conn| record_story(conn, story_id, &created, true))?;
                summary.created += 1;
                // The website starts every story with one empty chapter.
                remote_chapters = self.remote.list_chapters(&created.id).await?;
                created
            }
        };
        let remote_id = website.id.clone();
        if story_dirty && !skipped("story") {
            if website.fields.title != title || website.fields.description != description {
                let fields = StoryFields {
                    title,
                    description,
                    ..website.fields.clone()
                };
                website = self.save_story(story_id, &website, fields, true).await?;
                summary.updated += 1;
            } else {
                // Edited back to what the website already has.
                self.db(|conn| record_story(conn, story_id, &website, true))?;
            }
        }

        // A story this app has never sent chapters for: its first chapter takes
        // the place of the empty one the website made.
        if chapters.iter().all(|chapter| chapter.remote_id.is_none()) {
            if let (Some(first), Some(empty)) = (
                chapters.first_mut(),
                remote_chapters.iter().find(|r| r.word_count == 0),
            ) {
                first.remote_id = Some(empty.id.clone());
                first.remote_revision = Some(empty.revision);
            }
        }

        // ── Images ───────────────────────────────────────────────────────────
        let mut hosted: HashMap<String, String> = self.db(|conn| {
            let mut stmt =
                conn.prepare("SELECT asset, url FROM asset_uploads WHERE remote_story_id = ?1")?;
            let rows = stmt.query_map([&remote_id], |row| Ok((row.get(0)?, row.get(1)?)))?;
            Ok(rows.collect::<Result<_, _>>()?)
        })?;
        let mut wanted: Vec<String> = chapters
            .iter()
            .filter(|chapter| !skipped(&format!("chapter:{}", chapter.id)))
            .flat_map(|chapter| asset_refs(chapter.body.as_str().unwrap_or("")))
            .filter(|name| !hosted.contains_key(*name))
            .map(str::to_string)
            .collect();
        wanted.sort();
        wanted.dedup();
        for (index, name) in wanted.iter().enumerate() {
            (self.progress)(format!("Uploading image {} of {}", index + 1, wanted.len()));
            let bytes = std::fs::read(self.assets_dir.join(name))?;
            let url = self.remote.upload_image(&remote_id, name, bytes).await?;
            self.db(|conn| {
                conn.execute(
                    "INSERT OR REPLACE INTO asset_uploads (asset, remote_story_id, url) VALUES (?1, ?2, ?3)",
                    params![name, remote_id, url],
                )?;
                Ok(())
            })?;
            hosted.insert(name.clone(), url);
            summary.images += 1;
        }

        // ── Characters and places ────────────────────────────────────────────
        for kind in [EntityKind::Character, EntityKind::Place] {
            let (locals, graves) = entities.remove(&kind).unwrap_or_default();
            let theirs = remote_entities.remove(&kind).unwrap_or_default();
            if !graves.is_empty()
                || locals
                    .iter()
                    .any(|local| local.dirty || local.remote_id.is_none())
            {
                (self.progress)(format!("Sending {}", kind.path()));
            }
            for grave in graves {
                let remote = theirs.iter().find(|r| r.id == grave.remote_id);
                if let Some(remote) = remote.filter(|_| {
                    !skipped(&format!("deleted:{}:{}", kind.tombstone(), grave.remote_id))
                }) {
                    self.remote
                        .delete_entity(&remote_id, kind, &remote.id, remote.revision)
                        .await?;
                    summary.deleted += 1;
                }
                self.db(|conn| drop_tombstone(conn, kind.tombstone(), &grave.remote_id))?;
            }
            for local in locals {
                // A record removed on the website is sent again as new.
                let remote = theirs
                    .iter()
                    .find(|r| Some(&r.id) == local.remote_id.as_ref());
                let saved = match remote {
                    None => {
                        summary.created += 1;
                        self.remote
                            .create_entity(&remote_id, kind, &entity_input(kind, &local, None))
                            .await?
                    }
                    Some(remote)
                        if local.dirty
                            && !skipped(&format!("{}:{}", kind.tombstone(), local.id)) =>
                    {
                        summary.updated += 1;
                        let input = entity_input(kind, &local, Some(remote));
                        self.remote
                            .update_entity(&remote_id, kind, &remote.id, remote.revision, &input)
                            .await?
                    }
                    Some(_) => continue,
                };
                self.db(|conn| record(conn, kind.table(), &local, &saved.id, saved.revision))?;
            }
        }

        // ── Chapters ─────────────────────────────────────────────────────────
        for grave in chapter_graves {
            let remote = remote_chapters.iter().position(|r| r.id == grave.remote_id);
            if let Some(index) =
                remote.filter(|_| !skipped(&format!("deleted:chapter:{}", grave.remote_id)))
            {
                let gone = remote_chapters.remove(index);
                self.remote
                    .delete_chapter(&remote_id, &gone.id, gone.revision)
                    .await?;
                summary.deleted += 1;
            }
            self.db(|conn| drop_tombstone(conn, "chapter", &grave.remote_id))?;
        }
        // A skipped chapter stays out of the running order entirely.
        chapters.retain(|chapter| !skipped(&format!("chapter:{}", chapter.id)));
        let theirs: Vec<Option<&RemoteChapter>> = chapters
            .iter()
            .map(|chapter| {
                remote_chapters
                    .iter()
                    .find(|r| Some(&r.id) == chapter.remote_id.as_ref())
            })
            .collect();
        let current: Vec<Option<f64>> = theirs
            .iter()
            .map(|remote| remote.map(|r| r.position))
            .collect();
        let target = positions(
            &current,
            remote_chapters.iter().map(|r| r.position).collect(),
        );
        let total = chapters.len();
        let mut live: Vec<RemoteChapter> = Vec::with_capacity(total);
        for (index, chapter) in chapters.iter().enumerate() {
            let fields = || ChapterFields {
                title: chapter.label.clone(),
                content: with_hosted_images(chapter.body.as_str().unwrap_or(""), |name| {
                    hosted.get(name).cloned()
                }),
                position: target[index],
            };
            let saved = match theirs[index] {
                None => {
                    (self.progress)(format!("Sending chapter {} of {total}", index + 1));
                    summary.created += 1;
                    self.remote.create_chapter(&remote_id, &fields()).await?
                }
                Some(remote) => {
                    let changed_there = Some(remote.revision) != chapter.remote_revision;
                    if !chapter.dirty && !changed_there && remote.position == target[index] {
                        live.push(remote.clone());
                        continue;
                    }
                    (self.progress)(format!("Sending chapter {} of {total}", index + 1));
                    summary.updated += 1;
                    self.remote
                        .update_chapter(&remote_id, &remote.id, remote.revision, &fields())
                        .await?
                }
            };
            self.db(|conn| record(conn, CHAPTERS, chapter, &saved.id, saved.revision))?;
            live.push(saved);
        }

        // ── Release ──────────────────────────────────────────────────────────
        if mode == Mode::Publish {
            for chapter in live
                .iter()
                .filter(|chapter| chapter.word_count > 0 && !chapter.is_released())
            {
                self.remote
                    .release_chapter(&remote_id, &chapter.id, chapter.revision)
                    .await?;
                summary.released += 1;
            }
            if !website.fields.published {
                (self.progress)("Making the story public".into());
                let fields = StoryFields {
                    published: true,
                    ..website.fields.clone()
                };
                website = self.save_story(story_id, &website, fields, false).await?;
            }
        }
        Ok(Outcome::Done {
            remote_id,
            public: website.fields.published,
            summary,
        })
    }

    /// Updates the website story and records its new revision. `carried_edits`
    /// says whether this update sent the local title and description.
    async fn save_story(
        &self,
        story_id: &str,
        current: &RemoteStory,
        fields: StoryFields,
        carried_edits: bool,
    ) -> AppResult<RemoteStory> {
        let saved = self
            .remote
            .update_story(&current.id, current.revision, &fields)
            .await?;
        self.db(|conn| record_story(conn, story_id, &saved, carried_edits))?;
        Ok(saved)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store;
    use crate::sync::fake::{Desk, FakeRemote, Site};

    #[test]
    fn a_first_publish_sends_everything_and_a_second_sends_nothing() {
        let desk = Desk::new();
        let site = FakeRemote::default();

        let summary = desk.done(&site, Mode::Draft);
        assert_eq!(
            summary,
            Summary {
                created: 4,
                updated: 1,
                deleted: 0,
                images: 1,
                released: 0
            }
        );
        // The website's starter chapter was reused, not left behind as a blank.
        assert_eq!(site.titles(), ["One", "Two", "Three"]);
        {
            let live = site.site();
            assert!(live
                .chapters
                .iter()
                .any(|c| c.id == "starter" && c.title == "One"));
            assert!(live.chapters[0].content.contains("https://img.example/"));
            assert!(!live.chapters[0].content.contains("tt-asset:"));
            assert!(!live.story.as_ref().unwrap().fields.published);
            assert!(live.chapters.iter().all(|c| !c.released));
            assert_eq!(live.entities.len(), 1);
        }
        let plan = plan::preview(&desk.conn(), desk.assets.path(), &desk.story).unwrap();
        assert!(plan.linked && plan.nothing_to_send, "{plan:?}");

        let writes = site.site().writes;
        assert_eq!(desk.done(&site, Mode::Draft), Summary::default());
        assert_eq!(
            site.site().writes,
            writes,
            "an unchanged story costs no writes"
        );
    }

    #[test]
    fn publishing_releases_written_chapters_and_makes_the_story_public() {
        let desk = Desk::new();
        let site = FakeRemote::default();

        let outcome = desk.publish(&site, Mode::Publish, &[]).unwrap();
        assert!(matches!(outcome, Outcome::Done { public: true, .. }));
        let live = site.site();
        assert!(live.story.as_ref().unwrap().fields.published);
        let released: Vec<&str> = live
            .chapters
            .iter()
            .filter(|c| c.released)
            .map(|c| c.title.as_str())
            .collect();
        assert_eq!(released, ["One", "Two"], "the empty chapter stays a draft");
    }

    #[test]
    fn edits_reorders_deletions_and_additions_reach_the_website() {
        let mut desk = Desk::new();
        let site = FakeRemote::default();
        desk.done(&site, Mode::Draft);

        let [one, two, three] = [
            desk.chapters[0].clone(),
            desk.chapters[1].clone(),
            desk.chapters[2].clone(),
        ];
        let now = desk.tick();
        store::save_chapter_content(&mut desk.conn(), &two, "<p>second, revised</p>", now).unwrap();
        store::delete_chapter(&desk.conn(), &one).unwrap();
        let four = store::create_chapter(&mut desk.conn(), &desk.story, "Four", now)
            .unwrap()
            .summary
            .id;
        store::reorder_chapters(&mut desk.conn(), &desk.story, &[four, three, two], now).unwrap();
        store::update_story(&desk.conn(), &desk.story, "Tale, retitled", "", now).unwrap();

        let summary = desk.done(&site, Mode::Draft);
        assert_eq!(summary.deleted, 1);
        assert_eq!(site.titles(), ["Four", "Three", "Two"]);
        let live = site.site();
        assert_eq!(live.story.as_ref().unwrap().fields.title, "Tale, retitled");
        assert!(live
            .chapters
            .iter()
            .any(|c| c.content == "<p>second, revised</p>"));
    }

    #[test]
    fn an_interrupted_publish_finishes_when_run_again_without_duplicates() {
        // Break the connection after every possible number of writes.
        for limit in 0..12 {
            let desk = Desk::new();
            let site = FakeRemote::default();
            site.site().fail_after = Some(limit);
            let first = desk.publish(&site, Mode::Publish, &[]);

            site.site().fail_after = None;
            if first.is_err() {
                desk.done(&site, Mode::Publish);
            }
            assert_eq!(
                site.titles(),
                ["One", "Two", "Three"],
                "after failing at write {limit}"
            );
            let live = site.site();
            assert_eq!(live.entities.len(), 1, "after failing at write {limit}");
            assert_eq!(live.images.len(), 1, "after failing at write {limit}");
            assert!(live.story.as_ref().unwrap().fields.published);
        }
    }

    #[test]
    fn a_change_on_the_website_stops_the_publish_until_the_writer_chooses() {
        let mut desk = Desk::new();
        let site = FakeRemote::default();
        desk.done(&site, Mode::Draft);
        let two = desk.chapters[1].clone();
        let conflict = format!("chapter:{two}");

        // The same chapter is edited in both places.
        {
            let mut live = site.site();
            let theirs = live.chapters.iter_mut().find(|c| c.title == "Two").unwrap();
            theirs.content = "<p>edited on the website</p>".into();
            theirs.revision += 1;
        }
        let now = desk.tick();
        store::save_chapter_content(&mut desk.conn(), &two, "<p>edited here</p>", now).unwrap();

        let writes = site.site().writes;
        let outcome = desk.publish(&site, Mode::Draft, &[]).unwrap();
        let Outcome::Conflicts { conflicts } = outcome else {
            panic!("expected a conflict")
        };
        assert_eq!(conflicts.len(), 1);
        assert_eq!(
            (conflicts[0].id.as_str(), conflicts[0].label.as_str()),
            (conflict.as_str(), "Two")
        );
        assert_eq!(
            site.site().writes,
            writes,
            "nothing is written before the writer answers"
        );

        desk.publish(&site, Mode::Draft, &[(&conflict, Choice::Skip)])
            .unwrap();
        assert!(site
            .site()
            .chapters
            .iter()
            .any(|c| c.content == "<p>edited on the website</p>"));

        desk.publish(&site, Mode::Draft, &[(&conflict, Choice::Overwrite)])
            .unwrap();
        assert!(site
            .site()
            .chapters
            .iter()
            .any(|c| c.content == "<p>edited here</p>"));
        // Settled: the next publish has nothing to ask.
        assert_eq!(desk.done(&site, Mode::Draft), Summary::default());
    }

    #[test]
    fn overwriting_a_character_keeps_what_only_the_website_edits() {
        let mut desk = Desk::new();
        let site = FakeRemote::default();
        desk.done(&site, Mode::Draft);

        {
            let mut live = site.site();
            let (_, theirs) = &mut live.entities[0];
            theirs.body["relationships"] =
                json!([{"characterId": "c2", "name": "Marco", "type": "rival"}]);
            theirs.body["artUrl"] = json!("https://img.example/elena.png");
            theirs.revision += 1;
        }
        let mut character = desk.character.clone();
        character.notes = "Afraid of the sea.".into();
        let now = desk.tick();
        store::save_character(&desk.conn(), character, now).unwrap();
        let conflict = format!("character:{}", desk.character.id);

        assert!(matches!(
            desk.publish(&site, Mode::Draft, &[]).unwrap(),
            Outcome::Conflicts { .. }
        ));
        desk.publish(&site, Mode::Draft, &[(&conflict, Choice::Overwrite)])
            .unwrap();

        let live = site.site();
        let body = &live.entities[0].1.body;
        assert_eq!(body["notes"], "Afraid of the sea.");
        assert_eq!(body["relationships"][0]["name"], "Marco");
        assert_eq!(body["artUrl"], "https://img.example/elena.png");
    }

    #[test]
    fn a_story_removed_from_the_website_is_only_recreated_when_asked() {
        let desk = Desk::new();
        let site = FakeRemote::default();
        desk.done(&site, Mode::Draft);
        *site.site() = Site::default();

        assert_eq!(
            desk.publish(&site, Mode::Draft, &[]).unwrap(),
            Outcome::Gone
        );
        assert!(site.site().story.is_none());

        desk.publish(&site, Mode::Draft, &[(RECREATE, Choice::Overwrite)])
            .unwrap();
        assert_eq!(site.titles(), ["One", "Two", "Three"]);
        assert_eq!(
            site.site().images.len(),
            1,
            "images are uploaded for the new story"
        );
    }

    #[test]
    fn nothing_is_sent_without_the_agreement_or_past_a_limit() {
        let mut desk = Desk::new();
        let site = FakeRemote::default();

        site.site().declined_agreement = true;
        assert_eq!(
            desk.publish(&site, Mode::Publish, &[]).unwrap(),
            Outcome::NeedsAgreement
        );

        site.site().declined_agreement = false;
        let long = format!("<p>{}</p>", "word ".repeat(5_001));
        let (two, now) = (desk.chapters[1].clone(), desk.tick());
        store::save_chapter_content(&mut desk.conn(), &two, &long, now).unwrap();
        assert!(matches!(
            desk.publish(&site, Mode::Publish, &[]).unwrap(),
            Outcome::Blocked { .. }
        ));
        assert_eq!(site.site().writes, 0);
    }

    #[test]
    fn positions_follow_local_order_and_never_reuse_a_taken_value() {
        let strictly_rising = |values: &[f64]| values.windows(2).all(|pair| pair[0] < pair[1]);

        // Last chapter moved to the front.
        let moved = positions(&[Some(2.0), Some(0.0), Some(1.0)], vec![0.0, 1.0, 2.0]);
        assert!(strictly_rising(&moved), "{moved:?}");
        assert_eq!(moved[0], 2.0, "a chapter already in place is not rewritten");
        assert!(!moved[1..].iter().any(|p| [0.0, 1.0, 2.0].contains(p)));

        // Unchanged order costs nothing; new chapters slot in between and after.
        assert_eq!(
            positions(&[Some(0.0), Some(1.0)], vec![0.0, 1.0]),
            [0.0, 1.0]
        );
        assert_eq!(
            positions(&[Some(0.0), None, Some(1.0), None], vec![0.0, 1.0]),
            [0.0, 0.5, 1.0, 2.0]
        );

        // Fifty inserts into one gap run out of room and move past the end instead.
        let mut order = vec![Some(0.0)];
        order.extend(std::iter::repeat_n(None, 50));
        order.push(Some(1.0));
        let dense = positions(&order, vec![0.0, 1.0]);
        assert!(strictly_rising(&dense), "{dense:?}");
    }
}
