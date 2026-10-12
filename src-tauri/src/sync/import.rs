//! Bringing a website story onto this computer.
//!
//! An import is a one-time copy, not a link that stays in sync. The story is
//! remembered as the same website story, so publishing later updates it. Only
//! the writing comes across: chapters, characters and places. Everything is
//! downloaded first and then written in one transaction, so a failed import
//! leaves nothing half-made.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use rusqlite::types::Value as Sql;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::remote::{ChapterText, EntityKind, Remote, RemoteEntity, RemoteStory};
use super::SyncError;
use crate::assets;
use crate::db::new_id;
use crate::error::AppResult;
use crate::store::{overwrite_chapter_content, word_count};

/// What to do when the story is already on this computer.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Existing {
    /// Make the local story match the website. Replaced text is kept as a version.
    Replace,
    /// Add the website's version as a separate story that is not connected to the website.
    Copy,
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(
    tag = "status",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Imported {
    Done {
        story_id: String,
        chapters: usize,
        characters: usize,
        places: usize,
        images: usize,
        /// Images that could not be downloaded. They are untouched on the website.
        images_missing: usize,
        /// Chapters that exist only on this computer and were kept.
        kept_local: usize,
        /// False for a separate copy, which publishes as a new story.
        linked: bool,
    },
    /// The story is already here; the writer has to choose replace or copy.
    AlreadyHere { story_id: String, title: String },
}

const CHARACTER_TEXT: [&str; 6] = [
    "soul",
    "personality",
    "voice",
    "backstory",
    "affiliations",
    "notes",
];
const PLACE_TEXT: [&str; 6] = [
    "description",
    "atmosphere",
    "geography",
    "history",
    "significance",
    "notes",
];

fn text_fields(kind: EntityKind) -> &'static [&'static str] {
    match kind {
        EntityKind::Character => &CHARACTER_TEXT,
        EntityKind::Place => &PLACE_TEXT,
    }
}

/// The `src` of each image in chapter HTML, as written (with `&amp;` for `&`).
fn image_sources(html: &str) -> Vec<&str> {
    html.match_indices("src=\"")
        .filter_map(|(at, marker)| {
            let rest = &html[at + marker.len()..];
            rest.find('"').map(|end| &rest[..end])
        })
        .collect()
}

/// Column values for a character or place, in the order `columns` gives.
fn entity_values(kind: EntityKind, body: &Value) -> Vec<Sql> {
    let text = |field: &str| Sql::Text(body[field].as_str().unwrap_or("").to_string());
    let mut values = vec![text("name")];
    if kind == EntityKind::Character {
        values.push(body["age"].as_i64().map_or(Sql::Null, Sql::Integer));
    }
    values.extend(text_fields(kind).iter().map(|field| text(field)));
    values
}

fn columns(kind: EntityKind) -> Vec<&'static str> {
    let mut columns = vec!["name"];
    if kind == EntityKind::Character {
        columns.push("age");
    }
    columns.extend(text_fields(kind));
    columns
}

struct Download {
    story: RemoteStory,
    chapters: Vec<ChapterText>,
    entities: Vec<(EntityKind, Vec<RemoteEntity>)>,
    /// Local asset name and the website URL it came from.
    images: Vec<(String, String)>,
    images_missing: usize,
}

pub struct Importer<'a, R: Remote> {
    pub db: &'a Mutex<Connection>,
    pub assets_dir: &'a Path,
    pub remote: &'a R,
    /// The signed-in account; only its own stories are imported.
    pub uid: &'a str,
    pub progress: &'a (dyn Fn(String) + Send + Sync),
}

impl<R: Remote> Importer<'_, R> {
    fn db<T>(&self, work: impl FnOnce(&mut Connection) -> AppResult<T>) -> AppResult<T> {
        work(
            &mut self
                .db
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
        )
    }

    pub async fn import(
        &self,
        remote_id: &str,
        existing: Option<Existing>,
        now: i64,
    ) -> AppResult<Imported> {
        // story-data serves a published story to any signed-in caller, so a
        // successful read is not proof of ownership.
        let story = self.remote.get_story(remote_id).await?;
        if story.owner_id != self.uid {
            return Err(SyncError::NotFound.into());
        }
        let here: Option<(String, String)> = self.db(|conn| {
            Ok(conn
                .query_row(
                    "SELECT id, title FROM stories WHERE remote_id = ?1",
                    [remote_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?)
        })?;
        let target = match (here, existing) {
            (Some((story_id, title)), None) => {
                return Ok(Imported::AlreadyHere { story_id, title })
            }
            (Some((story_id, _)), Some(Existing::Replace)) => Some(story_id),
            _ => None,
        };
        let linked = existing != Some(Existing::Copy);

        let download = self.download(story).await?;
        let counts = (
            download.chapters.len(),
            download.entities[0].1.len(),
            download.entities[1].1.len(),
            download.images.len(),
            download.images_missing,
        );
        let (story_id, kept_local) = self.db(|conn| {
            let tx = conn.transaction()?;
            let written = match &target {
                Some(story_id) => (story_id.clone(), replace(&tx, story_id, &download, now)?),
                None => (create(&tx, &download, linked, now)?, 0),
            };
            if linked {
                for (asset, url) in &download.images {
                    tx.execute(
                        "INSERT OR REPLACE INTO asset_uploads (asset, remote_story_id, url) VALUES (?1, ?2, ?3)",
                        params![asset, download.story.id, url],
                    )?;
                }
            }
            tx.commit()?;
            Ok(written)
        })?;

        Ok(Imported::Done {
            story_id,
            chapters: counts.0,
            characters: counts.1,
            places: counts.2,
            images: counts.3,
            images_missing: counts.4,
            kept_local,
            linked,
        })
    }

    /// Reads the whole story from the website and saves its images locally,
    /// pointing each chapter's HTML at the local copy.
    async fn download(&self, story: RemoteStory) -> AppResult<Download> {
        (self.progress)("Downloading chapters".into());
        let mut chapters = self.remote.read_chapters(&story.id).await?;
        chapters.sort_by(|a, b| a.position.total_cmp(&b.position));
        let mut entities = Vec::new();
        for kind in [EntityKind::Character, EntityKind::Place] {
            entities.push((kind, self.remote.list_entities(&story.id, kind).await?));
        }

        // Keyed by the address as written in the HTML; `None` is a failed download.
        let mut saved: HashMap<String, Option<String>> = HashMap::new();
        let mut images = Vec::new();
        for chapter in &chapters {
            for written in image_sources(&chapter.content) {
                let url = written.replace("&amp;", "&");
                if saved.contains_key(written) || !self.remote.hosts_image(&url) {
                    continue;
                }
                (self.progress)(format!("Downloading image {}", saved.len() + 1));
                let name = match self.remote.download_image(&url).await {
                    Ok(bytes) => assets::save_image(self.assets_dir, &bytes).ok(),
                    Err(_) => None,
                };
                if let Some(name) = &name {
                    images.push((name.clone(), url));
                }
                saved.insert(written.to_string(), name);
            }
        }
        for chapter in &mut chapters {
            for (written, name) in &saved {
                if let Some(name) = name {
                    let from = format!("src=\"{written}\"");
                    chapter.content = chapter
                        .content
                        .replace(&from, &format!("src=\"tt-asset:{name}\""));
                }
            }
        }
        let images_missing = saved.values().filter(|name| name.is_none()).count();
        Ok(Download {
            story,
            chapters,
            entities,
            images,
            images_missing,
        })
    }
}

/// Adds a website chapter as a new local one and returns the local id.
fn insert_chapter(
    tx: &Transaction,
    story_id: &str,
    chapter: &ChapterText,
    position: usize,
    linked: bool,
    now: i64,
) -> AppResult<String> {
    let id = new_id();
    tx.execute(
        "INSERT INTO chapters (id, story_id, title, content, word_count, position, created_at, updated_at,
            remote_id, remote_revision, dirty)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, ?8, ?9, ?10)",
        params![
            id, story_id, chapter.title, chapter.content, word_count(&chapter.content), position as i64, now,
            linked.then_some(&chapter.id), linked.then_some(chapter.revision), !linked
        ],
    )?;
    Ok(id)
}

/// Adds a website character or place as a new local one and returns the local id.
fn insert_entity(
    tx: &Transaction,
    story_id: &str,
    kind: EntityKind,
    entity: &RemoteEntity,
    linked: bool,
    now: i64,
) -> AppResult<String> {
    let id = new_id();
    let mut names = columns(kind);
    names.extend([
        "id",
        "story_id",
        "created_at",
        "updated_at",
        "remote_id",
        "remote_revision",
        "dirty",
    ]);
    let mut values = entity_values(kind, &entity.body);
    values.extend([
        Sql::Text(id.clone()),
        Sql::Text(story_id.to_string()),
        Sql::Integer(now),
        Sql::Integer(now),
        if linked {
            Sql::Text(entity.id.clone())
        } else {
            Sql::Null
        },
        if linked {
            Sql::Integer(entity.revision)
        } else {
            Sql::Null
        },
        Sql::Integer(i64::from(!linked)),
    ]);
    tx.execute(
        &format!(
            "INSERT INTO {} ({}) VALUES ({})",
            kind.table(),
            names.join(", "),
            vec!["?"; names.len()].join(", ")
        ),
        params_from_iter(values),
    )?;
    Ok(id)
}

/// A new local story holding the website's version.
fn create(tx: &Transaction, download: &Download, linked: bool, now: i64) -> AppResult<String> {
    let story = &download.story;
    let story_id = new_id();
    let title = if linked {
        story.fields.title.clone()
    } else {
        format!("{} (website copy)", story.fields.title)
    };
    tx.execute(
        "INSERT INTO stories (id, title, description, created_at, updated_at, remote_id, remote_revision, dirty)
         VALUES (?1, ?2, ?3, ?4, ?4, ?5, ?6, ?7)",
        params![story_id, title, story.fields.description, now, linked.then_some(&story.id), linked.then_some(story.revision), !linked],
    )?;
    for (position, chapter) in download.chapters.iter().enumerate() {
        insert_chapter(tx, &story_id, chapter, position, linked, now)?;
    }
    for (kind, entities) in &download.entities {
        for entity in entities {
            insert_entity(tx, &story_id, *kind, entity, linked, now)?;
        }
    }
    Ok(story_id)
}

/// Makes an existing local story match the website. Text that is replaced is
/// kept as a version, and writing that exists only here is never deleted: it
/// stays, after the website's chapters, as unpublished work. Returns how many
/// such chapters were kept.
fn replace(tx: &Transaction, story_id: &str, download: &Download, now: i64) -> AppResult<usize> {
    let story = &download.story;
    tx.execute(
        "UPDATE stories SET title = ?2, description = ?3, updated_at = ?4, remote_revision = ?5, dirty = 0 WHERE id = ?1",
        params![story_id, story.fields.title, story.fields.description, now, story.revision],
    )?;
    // The website's copy wins, including anything deleted here but still there.
    tx.execute("DELETE FROM tombstones WHERE story_id = ?1", [story_id])?;

    let local_for = |table: &str, remote: &str| -> AppResult<Option<String>> {
        Ok(tx
            .query_row(
                &format!("SELECT id FROM {table} WHERE story_id = ?1 AND remote_id = ?2"),
                [story_id, remote],
                |row| row.get(0),
            )
            .optional()?)
    };

    // Local chapters that now hold a website chapter, in the website's order.
    let mut from_website = Vec::new();
    for (position, chapter) in download.chapters.iter().enumerate() {
        from_website.push(match local_for("chapters", &chapter.id)? {
            Some(local) => {
                overwrite_chapter_content(tx, &local, &chapter.content, now)?;
                tx.execute(
                    "UPDATE chapters SET title = ?2, position = ?3, remote_revision = ?4, dirty = 0 WHERE id = ?1",
                    params![local, chapter.title, position as i64, chapter.revision],
                )?;
                local
            }
            None => insert_chapter(tx, story_id, chapter, position, true, now)?,
        });
    }
    // Chapters the website does not have: never published, or deleted there.
    let only_here: Vec<String> = tx
        .prepare("SELECT id FROM chapters WHERE story_id = ?1 ORDER BY position, created_at")?
        .query_map([story_id], |row| row.get(0))?
        .collect::<Result<Vec<String>, _>>()?
        .into_iter()
        .filter(|id| !from_website.contains(id))
        .collect();
    for (offset, id) in only_here.iter().enumerate() {
        tx.execute(
            "UPDATE chapters SET remote_id = NULL, remote_revision = NULL, dirty = 1, position = ?2 WHERE id = ?1",
            params![id, (download.chapters.len() + offset) as i64],
        )?;
    }

    for (kind, entities) in &download.entities {
        let mut from_website = Vec::new();
        for entity in entities {
            from_website.push(match local_for(kind.table(), &entity.id)? {
                Some(local) => {
                    let assignments: Vec<String> = columns(*kind).iter().map(|column| format!("{column} = ?")).collect();
                    let mut values = entity_values(*kind, &entity.body);
                    values.extend([Sql::Integer(now), Sql::Integer(entity.revision), Sql::Text(local.clone())]);
                    tx.execute(
                        &format!(
                            "UPDATE {} SET {}, updated_at = ?, remote_revision = ?, dirty = 0 WHERE id = ?",
                            kind.table(),
                            assignments.join(", ")
                        ),
                        params_from_iter(values),
                    )?;
                    local
                }
                None => insert_entity(tx, story_id, *kind, entity, true, now)?,
            });
        }
        // Anything else was never published or is gone from the website: keep it, unpublished.
        let mut values = vec![Sql::Text(story_id.to_string())];
        values.extend(from_website.iter().cloned().map(Sql::Text));
        tx.execute(
            &format!(
                "UPDATE {} SET remote_id = NULL, remote_revision = NULL, dirty = 1
                 WHERE story_id = ? AND id NOT IN ({})",
                kind.table(),
                vec!["?"; from_website.len()].join(", ")
            ),
            params_from_iter(values),
        )?;
    }
    Ok(only_here.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;
    use crate::error::AppError;
    use crate::store;
    use crate::sync::fake::{Desk, FakeRemote, UID};
    use crate::sync::publish::{Mode, Summary};

    /// A second, empty computer.
    struct Laptop {
        db: Mutex<Connection>,
        assets: tempfile::TempDir,
    }

    impl Laptop {
        fn new() -> Self {
            Self {
                db: Mutex::new(open_in_memory().unwrap()),
                assets: tempfile::tempdir().unwrap(),
            }
        }

        fn import(&self, site: &FakeRemote, existing: Option<Existing>) -> AppResult<Imported> {
            let remote_id = site.site().story.as_ref().unwrap().id.clone();
            import_into(
                &self.db,
                self.assets.path(),
                site,
                &remote_id,
                existing,
                500,
            )
        }
    }

    fn import_into(
        db: &Mutex<Connection>,
        assets: &Path,
        site: &FakeRemote,
        remote_id: &str,
        existing: Option<Existing>,
        now: i64,
    ) -> AppResult<Imported> {
        let importer = Importer {
            db,
            assets_dir: assets,
            remote: site,
            uid: UID,
            progress: &|_| {},
        };
        tauri::async_runtime::block_on(importer.import(remote_id, existing, now))
    }

    /// A website story written and published from another computer.
    fn published_site() -> (Desk, FakeRemote) {
        let desk = Desk::new();
        let site = FakeRemote::default();
        desk.done(&site, Mode::Publish);
        (desk, site)
    }

    #[test]
    fn an_imported_story_opens_offline_and_publishes_back_as_no_change() {
        let (_, site) = published_site();
        let laptop = Laptop::new();

        let Imported::Done {
            story_id,
            chapters,
            characters,
            images,
            images_missing,
            linked,
            ..
        } = laptop.import(&site, None).unwrap()
        else {
            panic!("expected the import to finish")
        };
        assert_eq!(
            (chapters, characters, images, images_missing, linked),
            (3, 1, 1, 0, true)
        );

        let conn = laptop.db.lock().unwrap();
        let listed = store::list_chapters(&conn, &story_id).unwrap();
        let titles: Vec<&str> = listed.iter().map(|c| c.title.as_str()).collect();
        assert_eq!(titles, ["One", "Two", "Three"]);
        // The image is a local file now, so the chapter shows without a connection.
        let first = store::get_chapter(&conn, &listed[0].id).unwrap().content;
        let name = first
            .split("tt-asset:")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap();
        assert!(!first.contains("img.example"));
        assert!(laptop.assets.path().join(name).is_file());
        assert_eq!(
            store::list_characters(&conn, &story_id).unwrap()[0].name,
            "Elena"
        );
        drop(conn);

        // Publishing what was just imported must not touch the website.
        let writes = site.site().writes;
        let publisher = crate::sync::publish::Publisher {
            db: &laptop.db,
            assets_dir: laptop.assets.path(),
            remote: &site,
            uid: UID,
            progress: &|_| {},
        };
        let outcome = tauri::async_runtime::block_on(publisher.publish(
            &story_id,
            Mode::Publish,
            &Default::default(),
        ))
        .unwrap();
        assert!(
            matches!(outcome, crate::sync::publish::Outcome::Done { summary, .. } if summary == Summary::default())
        );
        assert_eq!(site.site().writes, writes);
    }

    #[test]
    fn a_story_owned_by_someone_else_is_refused() {
        let (_, site) = published_site();
        site.site().story.as_mut().unwrap().owner_id = "someone-else".into();
        let laptop = Laptop::new();

        let refused = laptop.import(&site, None);
        assert!(matches!(refused, Err(AppError::Sync(SyncError::NotFound))));
        let stories: i64 = laptop
            .db
            .lock()
            .unwrap()
            .query_row("SELECT count(*) FROM stories", [], |r| r.get(0))
            .unwrap();
        assert_eq!(stories, 0);
    }

    #[test]
    fn an_image_that_cannot_be_downloaded_is_reported_and_left_alone() {
        let (_, site) = published_site();
        site.site().files.clear();
        let laptop = Laptop::new();

        let Imported::Done {
            story_id,
            images,
            images_missing,
            ..
        } = laptop.import(&site, None).unwrap()
        else {
            panic!("expected the import to finish")
        };
        assert_eq!((images, images_missing), (0, 1));
        let conn = laptop.db.lock().unwrap();
        let first = store::list_chapters(&conn, &story_id).unwrap().remove(0);
        let content = store::get_chapter(&conn, &first.id).unwrap().content;
        assert!(
            content.contains("https://img.example/"),
            "the website address is kept as it was"
        );
    }

    #[test]
    fn replacing_takes_the_website_version_but_keeps_local_writing_recoverable() {
        let (mut desk, site) = published_site();
        let two = desk.chapters[1].clone();
        // Edited on the website, and differently on this computer.
        {
            let mut live = site.site();
            let theirs = live.chapters.iter_mut().find(|c| c.title == "Two").unwrap();
            theirs.content = "<p>from the website</p>".into();
            theirs.revision += 1;
        }
        let now = desk.tick();
        store::save_chapter_content(&mut desk.conn(), &two, "<p>written here</p>", now).unwrap();
        let extra = store::create_chapter(&mut desk.conn(), &desk.story, "Only here", now)
            .unwrap()
            .summary
            .id;
        let remote_id = site.site().story.as_ref().unwrap().id.clone();
        let import = |existing| {
            import_into(
                &desk.db,
                desk.assets.path(),
                &site,
                &remote_id,
                existing,
                900,
            )
        };

        // Already here: nothing happens until the writer chooses.
        assert_eq!(
            import(None).unwrap(),
            Imported::AlreadyHere {
                story_id: desk.story.clone(),
                title: "Tale".into()
            }
        );
        assert_eq!(
            store::get_chapter(&desk.conn(), &two).unwrap().content,
            "<p>written here</p>"
        );

        let Imported::Done {
            story_id,
            kept_local,
            ..
        } = import(Some(Existing::Replace)).unwrap()
        else {
            panic!("expected the import to finish")
        };
        assert_eq!((story_id.as_str(), kept_local), (desk.story.as_str(), 1));
        let conn = desk.conn();
        assert_eq!(
            store::get_chapter(&conn, &two).unwrap().content,
            "<p>from the website</p>"
        );
        let versions: Vec<String> = store::list_snapshots(&conn, &two)
            .unwrap()
            .iter()
            .map(|s| store::get_snapshot_content(&conn, &s.id).unwrap())
            .collect();
        assert!(
            versions.contains(&"<p>written here</p>".to_string()),
            "{versions:?}"
        );
        // The chapter the website never had is still there, last, as unpublished work.
        let listed = store::list_chapters(&conn, &desk.story).unwrap();
        assert_eq!(listed.last().unwrap().id, extra);
        assert_eq!(listed.len(), 4);
    }

    #[test]
    fn keeping_both_adds_a_separate_story_that_is_not_connected_to_the_website() {
        let (desk, site) = published_site();
        let remote_id = site.site().story.as_ref().unwrap().id.clone();

        let Imported::Done {
            story_id, linked, ..
        } = import_into(
            &desk.db,
            desk.assets.path(),
            &site,
            &remote_id,
            Some(Existing::Copy),
            900,
        )
        .unwrap()
        else {
            panic!("expected the import to finish")
        };
        assert!(!linked);
        assert_ne!(story_id, desk.story);
        let conn = desk.conn();
        let (title, remote): (String, Option<String>) = conn
            .query_row(
                "SELECT title, remote_id FROM stories WHERE id = ?1",
                [&story_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((title.as_str(), remote), ("Tale (website copy)", None));
        let linked_rows: i64 = conn
            .query_row(
                "SELECT count(*) FROM chapters WHERE story_id = ?1 AND remote_id IS NOT NULL",
                [&story_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            linked_rows, 0,
            "its chapters would publish as new, not over the original"
        );
    }
}
