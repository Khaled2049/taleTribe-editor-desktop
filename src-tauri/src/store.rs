//! All reads and writes of story data. Functions take the clock as an argument
//! so the snapshot rules can be tested without sleeping.

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::db::new_id;
use crate::error::{AppError, AppResult};

// Field bounds match story-data's `internal/store/validate.go`, so nothing
// saved here is rejected for its size at publish time.
const MAX_TITLE_CHARS: usize = 500;
const MAX_DESCRIPTION_CHARS: usize = 5_000;
const MAX_NAME_CHARS: usize = 200;
const MAX_PROSE_CHARS: usize = 20_000;

const SNAPSHOT_INTERVAL_MS: i64 = 5 * 60 * 1000;
const SNAPSHOTS_KEPT: i64 = 20;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Story {
    pub id: String,
    pub title: String,
    pub description: String,
    pub chapter_count: i64,
    pub word_count: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterSummary {
    pub id: String,
    pub story_id: String,
    pub title: String,
    pub word_count: i64,
    pub position: i64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Chapter {
    #[serde(flatten)]
    pub summary: ChapterSummary,
    pub content: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub id: String,
    pub chapter_id: String,
    pub word_count: i64,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Character {
    /// Empty on input means "create".
    #[serde(default)]
    pub id: String,
    pub story_id: String,
    pub name: String,
    pub age: Option<i64>,
    pub soul: String,
    pub personality: String,
    pub voice: String,
    pub backstory: String,
    pub affiliations: String,
    pub notes: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Place {
    /// Empty on input means "create".
    #[serde(default)]
    pub id: String,
    pub story_id: String,
    pub name: String,
    pub description: String,
    pub atmosphere: String,
    pub geography: String,
    pub history: String,
    pub significance: String,
    pub notes: String,
}

/// story-data's rule: runs of anything but space, newline, tab and carriage
/// return, counted over the stored HTML. Must stay identical to its `wordCount`.
pub fn word_count(html: &str) -> i64 {
    html.split([' ', '\n', '\t', '\r'])
        .filter(|run| !run.is_empty())
        .count() as i64
}

fn bounded(label: &str, value: &str, max_chars: usize) -> AppResult<()> {
    if value.chars().count() > max_chars {
        return Err(AppError::validation(format!(
            "{label} can be at most {max_chars} characters"
        )));
    }
    Ok(())
}

fn title_or(value: &str, fallback: &str) -> AppResult<String> {
    let title = value.trim();
    bounded("A title", title, MAX_TITLE_CHARS)?;
    Ok(if title.is_empty() { fallback } else { title }.to_string())
}

fn required_name(value: &str) -> AppResult<String> {
    let name = value.trim();
    if name.is_empty() {
        return Err(AppError::validation("A name is required"));
    }
    bounded("A name", name, MAX_NAME_CHARS)?;
    Ok(name.to_string())
}

fn found(changed: usize, what: &'static str) -> AppResult<()> {
    if changed == 0 {
        return Err(AppError::NotFound(what));
    }
    Ok(())
}

// ── Stories ──────────────────────────────────────────────────────────────────

const STORY_SELECT: &str = "SELECT s.id, s.title, s.description, s.created_at, s.updated_at,
        (SELECT count(*) FROM chapters c WHERE c.story_id = s.id),
        (SELECT coalesce(sum(c.word_count), 0) FROM chapters c WHERE c.story_id = s.id)
     FROM stories s";

fn story_from(row: &Row) -> rusqlite::Result<Story> {
    Ok(Story {
        id: row.get(0)?,
        title: row.get(1)?,
        description: row.get(2)?,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
        chapter_count: row.get(5)?,
        word_count: row.get(6)?,
    })
}

pub fn list_stories(conn: &Connection) -> AppResult<Vec<Story>> {
    let mut stmt = conn.prepare(&format!("{STORY_SELECT} ORDER BY s.updated_at DESC"))?;
    let rows = stmt.query_map([], story_from)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn get_story(conn: &Connection, id: &str) -> AppResult<Story> {
    conn.query_row(&format!("{STORY_SELECT} WHERE s.id = ?1"), [id], story_from)
        .optional()?
        .ok_or(AppError::NotFound("story"))
}

/// A new story starts with one empty chapter so the editor always has a page.
pub fn create_story(conn: &mut Connection, title: &str, now: i64) -> AppResult<Story> {
    let title = title_or(title, "Untitled story")?;
    let id = new_id();
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO stories (id, title, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
        params![id, title, now],
    )?;
    tx.execute(
        "INSERT INTO chapters (id, story_id, title, position, created_at, updated_at)
         VALUES (?1, ?2, 'Chapter 1', 0, ?3, ?3)",
        params![new_id(), id, now],
    )?;
    tx.commit()?;
    get_story(conn, &id)
}

pub fn update_story(
    conn: &Connection,
    id: &str,
    title: &str,
    description: &str,
    now: i64,
) -> AppResult<Story> {
    let title = title_or(title, "Untitled story")?;
    bounded("A description", description, MAX_DESCRIPTION_CHARS)?;
    let changed = conn.execute(
        "UPDATE stories SET title = ?2, description = ?3, updated_at = ?4, dirty = 1 WHERE id = ?1",
        params![id, title, description, now],
    )?;
    found(changed, "story")?;
    get_story(conn, id)
}

pub fn delete_story(conn: &Connection, id: &str) -> AppResult<()> {
    found(
        conn.execute("DELETE FROM stories WHERE id = ?1", [id])?,
        "story",
    )
}

/// Deletes a record. If it was published, a tombstone is left in the same
/// transaction so the next publish removes it from the website too.
fn delete_published(
    conn: &Connection,
    table: &str,
    kind: &'static str,
    label: &str,
    id: &str,
) -> AppResult<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        &format!(
            "INSERT OR REPLACE INTO tombstones (kind, remote_id, story_id, remote_revision, label)
             SELECT ?1, remote_id, story_id, remote_revision, {label} FROM {table}
             WHERE id = ?2 AND remote_id IS NOT NULL"
        ),
        params![kind, id],
    )?;
    let deleted = tx.execute(&format!("DELETE FROM {table} WHERE id = ?1"), [id])?;
    found(deleted, kind)?;
    Ok(tx.commit()?)
}

fn touch_story(conn: &Connection, story_id: &str, now: i64) -> AppResult<()> {
    conn.execute(
        "UPDATE stories SET updated_at = ?2 WHERE id = ?1",
        params![story_id, now],
    )?;
    Ok(())
}

// ── Chapters ─────────────────────────────────────────────────────────────────

const CHAPTER_COLUMNS: &str = "id, story_id, title, word_count, position, updated_at";

fn summary_from(row: &Row) -> rusqlite::Result<ChapterSummary> {
    Ok(ChapterSummary {
        id: row.get(0)?,
        story_id: row.get(1)?,
        title: row.get(2)?,
        word_count: row.get(3)?,
        position: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

pub fn list_chapters(conn: &Connection, story_id: &str) -> AppResult<Vec<ChapterSummary>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {CHAPTER_COLUMNS} FROM chapters WHERE story_id = ?1 ORDER BY position, created_at"
    ))?;
    let rows = stmt.query_map([story_id], summary_from)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn get_chapter(conn: &Connection, id: &str) -> AppResult<Chapter> {
    conn.query_row(
        &format!("SELECT {CHAPTER_COLUMNS}, content FROM chapters WHERE id = ?1"),
        [id],
        |row| {
            Ok(Chapter {
                summary: summary_from(row)?,
                content: row.get(6)?,
            })
        },
    )
    .optional()?
    .ok_or(AppError::NotFound("chapter"))
}

pub fn create_chapter(
    conn: &mut Connection,
    story_id: &str,
    title: &str,
    now: i64,
) -> AppResult<Chapter> {
    let tx = conn.transaction()?;
    let (exists, next): (bool, i64) = tx.query_row(
        "SELECT EXISTS (SELECT 1 FROM stories WHERE id = ?1),
                (SELECT coalesce(max(position) + 1, 0) FROM chapters WHERE story_id = ?1)",
        [story_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    if !exists {
        return Err(AppError::NotFound("story"));
    }
    let title = title_or(title, &format!("Chapter {}", next + 1))?;
    let id = new_id();
    tx.execute(
        "INSERT INTO chapters (id, story_id, title, position, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
        params![id, story_id, title, next, now],
    )?;
    touch_story(&tx, story_id, now)?;
    tx.commit()?;
    get_chapter(conn, &id)
}

pub fn rename_chapter(conn: &Connection, id: &str, title: &str, now: i64) -> AppResult<Chapter> {
    let title = title_or(title, "Untitled chapter")?;
    let changed = conn.execute(
        "UPDATE chapters SET title = ?2, updated_at = ?3, dirty = 1 WHERE id = ?1",
        params![id, title, now],
    )?;
    found(changed, "chapter")?;
    get_chapter(conn, id)
}

pub fn delete_chapter(conn: &Connection, id: &str) -> AppResult<()> {
    delete_published(conn, "chapters", "chapter", "title", id)
}

/// `ordered_ids` must be exactly the story's chapters; anything else is refused
/// so a stale list from the UI cannot drop or duplicate a position.
pub fn reorder_chapters(
    conn: &mut Connection,
    story_id: &str,
    ordered_ids: &[String],
    now: i64,
) -> AppResult<Vec<ChapterSummary>> {
    let tx = conn.transaction()?;
    let mut current: Vec<String> = tx
        .prepare("SELECT id FROM chapters WHERE story_id = ?1")?
        .query_map([story_id], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    let mut requested = ordered_ids.to_vec();
    current.sort();
    requested.sort();
    if current != requested {
        return Err(AppError::validation(
            "The chapter list changed. Reload and try again.",
        ));
    }
    for (position, id) in ordered_ids.iter().enumerate() {
        tx.execute(
            "UPDATE chapters SET position = ?2, dirty = 1 WHERE id = ?1 AND position <> ?2",
            params![id, position as i64],
        )?;
    }
    touch_story(&tx, story_id, now)?;
    tx.commit()?;
    list_chapters(conn, story_id)
}

/// Saves the chapter text. Before overwriting, the text being replaced is kept
/// as a snapshot if none has been taken for `SNAPSHOT_INTERVAL_MS`.
pub fn save_chapter_content(
    conn: &mut Connection,
    id: &str,
    content: &str,
    now: i64,
) -> AppResult<ChapterSummary> {
    let tx = conn.transaction()?;
    write_content(&tx, id, content, now, false)?;
    tx.commit()?;
    Ok(get_chapter(conn, id)?.summary)
}

/// Replaces a chapter's text with a version from elsewhere, always keeping the
/// text it had as a snapshot first.
pub fn overwrite_chapter_content(
    conn: &Connection,
    id: &str,
    content: &str,
    now: i64,
) -> AppResult<()> {
    write_content(conn, id, content, now, true)
}

fn write_content(
    conn: &Connection,
    id: &str,
    content: &str,
    now: i64,
    always_snapshot: bool,
) -> AppResult<()> {
    let (story_id, previous, previous_saved_at): (String, String, i64) = conn
        .query_row(
            "SELECT story_id, content, updated_at FROM chapters WHERE id = ?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?
        .ok_or(AppError::NotFound("chapter"))?;
    if previous == content {
        return Ok(());
    }

    if word_count(&previous) > 0 {
        let latest: Option<i64> = conn.query_row(
            "SELECT max(created_at) FROM chapter_snapshots WHERE chapter_id = ?1",
            [id],
            |row| row.get(0),
        )?;
        let due = latest.is_none_or(|at| now - at >= SNAPSHOT_INTERVAL_MS);
        if always_snapshot || due {
            // Dated when that text was last saved, which is what the writer recognises.
            conn.execute(
                "INSERT INTO chapter_snapshots (id, chapter_id, content, word_count, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    new_id(),
                    id,
                    previous,
                    word_count(&previous),
                    previous_saved_at
                ],
            )?;
            conn.execute(
                "DELETE FROM chapter_snapshots WHERE chapter_id = ?1 AND id NOT IN (
                    SELECT id FROM chapter_snapshots WHERE chapter_id = ?1
                    ORDER BY created_at DESC, rowid DESC LIMIT ?2)",
                params![id, SNAPSHOTS_KEPT],
            )?;
        }
    }

    conn.execute(
        "UPDATE chapters SET content = ?2, word_count = ?3, updated_at = ?4, dirty = 1 WHERE id = ?1",
        params![id, content, word_count(content), now],
    )?;
    touch_story(conn, &story_id, now)
}

// ── Snapshots ────────────────────────────────────────────────────────────────

pub fn list_snapshots(conn: &Connection, chapter_id: &str) -> AppResult<Vec<Snapshot>> {
    let mut stmt = conn.prepare(
        "SELECT id, chapter_id, word_count, created_at FROM chapter_snapshots
         WHERE chapter_id = ?1 ORDER BY created_at DESC, rowid DESC",
    )?;
    let rows = stmt.query_map([chapter_id], |row| {
        Ok(Snapshot {
            id: row.get(0)?,
            chapter_id: row.get(1)?,
            word_count: row.get(2)?,
            created_at: row.get(3)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn get_snapshot_content(conn: &Connection, id: &str) -> AppResult<String> {
    conn.query_row(
        "SELECT content FROM chapter_snapshots WHERE id = ?1",
        [id],
        |row| row.get(0),
    )
    .optional()?
    .ok_or(AppError::NotFound("version"))
}

/// Restoring always snapshots the current text first, so it can be undone.
pub fn restore_snapshot(conn: &mut Connection, snapshot_id: &str, now: i64) -> AppResult<Chapter> {
    let tx = conn.transaction()?;
    let (chapter_id, content): (String, String) = tx
        .query_row(
            "SELECT chapter_id, content FROM chapter_snapshots WHERE id = ?1",
            [snapshot_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?
        .ok_or(AppError::NotFound("version"))?;
    write_content(&tx, &chapter_id, &content, now, true)?;
    tx.commit()?;
    get_chapter(conn, &chapter_id)
}

// ── Characters and places ────────────────────────────────────────────────────

fn story_exists(conn: &Connection, story_id: &str) -> AppResult<()> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM stories WHERE id = ?1)",
        [story_id],
        |row| row.get(0),
    )?;
    if exists {
        Ok(())
    } else {
        Err(AppError::NotFound("story"))
    }
}

pub fn list_characters(conn: &Connection, story_id: &str) -> AppResult<Vec<Character>> {
    let mut stmt = conn.prepare(
        "SELECT id, story_id, name, age, soul, personality, voice, backstory, affiliations, notes
         FROM characters WHERE story_id = ?1 ORDER BY name COLLATE NOCASE",
    )?;
    let rows = stmt.query_map([story_id], |row| {
        Ok(Character {
            id: row.get(0)?,
            story_id: row.get(1)?,
            name: row.get(2)?,
            age: row.get(3)?,
            soul: row.get(4)?,
            personality: row.get(5)?,
            voice: row.get(6)?,
            backstory: row.get(7)?,
            affiliations: row.get(8)?,
            notes: row.get(9)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn save_character(conn: &Connection, mut input: Character, now: i64) -> AppResult<Character> {
    input.name = required_name(&input.name)?;
    for (label, value) in [
        ("Soul", &input.soul),
        ("Personality", &input.personality),
        ("Voice", &input.voice),
        ("Backstory", &input.backstory),
        ("Affiliations", &input.affiliations),
        ("Notes", &input.notes),
    ] {
        bounded(label, value, MAX_PROSE_CHARS)?;
    }
    if input.age.is_some_and(|age| !(0..=100_000).contains(&age)) {
        return Err(AppError::validation("Age must be a positive number"));
    }
    if input.id.is_empty() {
        story_exists(conn, &input.story_id)?;
        input.id = new_id();
        conn.execute(
            "INSERT INTO characters (id, story_id, name, age, soul, personality, voice,
                backstory, affiliations, notes, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)",
            params![
                input.id,
                input.story_id,
                input.name,
                input.age,
                input.soul,
                input.personality,
                input.voice,
                input.backstory,
                input.affiliations,
                input.notes,
                now
            ],
        )?;
    } else {
        let changed = conn.execute(
            "UPDATE characters SET name = ?2, age = ?3, soul = ?4, personality = ?5, voice = ?6,
                backstory = ?7, affiliations = ?8, notes = ?9, updated_at = ?10, dirty = 1
             WHERE id = ?1 AND story_id = ?11",
            params![
                input.id,
                input.name,
                input.age,
                input.soul,
                input.personality,
                input.voice,
                input.backstory,
                input.affiliations,
                input.notes,
                now,
                input.story_id
            ],
        )?;
        found(changed, "character")?;
    }
    touch_story(conn, &input.story_id, now)?;
    Ok(input)
}

pub fn delete_character(conn: &Connection, id: &str) -> AppResult<()> {
    delete_published(conn, "characters", "character", "name", id)
}

pub fn list_places(conn: &Connection, story_id: &str) -> AppResult<Vec<Place>> {
    let mut stmt = conn.prepare(
        "SELECT id, story_id, name, description, atmosphere, geography, history, significance, notes
         FROM places WHERE story_id = ?1 ORDER BY name COLLATE NOCASE",
    )?;
    let rows = stmt.query_map([story_id], |row| {
        Ok(Place {
            id: row.get(0)?,
            story_id: row.get(1)?,
            name: row.get(2)?,
            description: row.get(3)?,
            atmosphere: row.get(4)?,
            geography: row.get(5)?,
            history: row.get(6)?,
            significance: row.get(7)?,
            notes: row.get(8)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn save_place(conn: &Connection, mut input: Place, now: i64) -> AppResult<Place> {
    input.name = required_name(&input.name)?;
    for (label, value) in [
        ("Description", &input.description),
        ("Atmosphere", &input.atmosphere),
        ("Geography", &input.geography),
        ("History", &input.history),
        ("Significance", &input.significance),
        ("Notes", &input.notes),
    ] {
        bounded(label, value, MAX_PROSE_CHARS)?;
    }
    if input.id.is_empty() {
        story_exists(conn, &input.story_id)?;
        input.id = new_id();
        conn.execute(
            "INSERT INTO places (id, story_id, name, description, atmosphere, geography,
                history, significance, notes, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
            params![
                input.id,
                input.story_id,
                input.name,
                input.description,
                input.atmosphere,
                input.geography,
                input.history,
                input.significance,
                input.notes,
                now
            ],
        )?;
    } else {
        let changed = conn.execute(
            "UPDATE places SET name = ?2, description = ?3, atmosphere = ?4, geography = ?5,
                history = ?6, significance = ?7, notes = ?8, updated_at = ?9, dirty = 1
             WHERE id = ?1 AND story_id = ?10",
            params![
                input.id,
                input.name,
                input.description,
                input.atmosphere,
                input.geography,
                input.history,
                input.significance,
                input.notes,
                now,
                input.story_id
            ],
        )?;
        found(changed, "place")?;
    }
    touch_story(conn, &input.story_id, now)?;
    Ok(input)
}

pub fn delete_place(conn: &Connection, id: &str) -> AppResult<()> {
    delete_published(conn, "places", "place", "name", id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;

    const MINUTE: i64 = 60 * 1000;

    /// A story with its first chapter, created at time 0.
    fn story_with_chapter(conn: &mut Connection) -> (String, String) {
        let story = create_story(conn, "Draft", 0).unwrap();
        let chapter = list_chapters(conn, &story.id).unwrap().remove(0);
        (story.id, chapter.id)
    }

    #[test]
    fn word_count_matches_story_data_fixtures() {
        #[derive(Deserialize)]
        struct Case {
            html: String,
            words: i64,
        }
        let cases: Vec<Case> =
            serde_json::from_str(include_str!("../../fixtures/word-count.json")).unwrap();
        for case in cases {
            assert_eq!(word_count(&case.html), case.words, "html: {:?}", case.html);
        }
    }

    #[test]
    fn saving_keeps_a_snapshot_of_replaced_text_at_most_once_per_interval() {
        let mut conn = open_in_memory().unwrap();
        let (_, chapter) = story_with_chapter(&mut conn);

        // Nothing to keep when the chapter was empty.
        save_chapter_content(&mut conn, &chapter, "<p>one</p>", MINUTE).unwrap();
        assert!(list_snapshots(&conn, &chapter).unwrap().is_empty());

        save_chapter_content(&mut conn, &chapter, "<p>one two</p>", 2 * MINUTE).unwrap();
        save_chapter_content(&mut conn, &chapter, "<p>one two three</p>", 3 * MINUTE).unwrap();
        let snapshots = list_snapshots(&conn, &chapter).unwrap();
        assert_eq!(snapshots.len(), 1, "second save falls inside the interval");
        assert_eq!(
            get_snapshot_content(&conn, &snapshots[0].id).unwrap(),
            "<p>one</p>"
        );

        save_chapter_content(&mut conn, &chapter, "<p>later</p>", 10 * MINUTE).unwrap();
        assert_eq!(list_snapshots(&conn, &chapter).unwrap().len(), 2);
    }

    #[test]
    fn snapshots_are_pruned_to_the_newest_twenty() {
        let mut conn = open_in_memory().unwrap();
        let (_, chapter) = story_with_chapter(&mut conn);
        for i in 0..30 {
            let text = format!("<p>version {i}</p>");
            save_chapter_content(&mut conn, &chapter, &text, i * 10 * MINUTE).unwrap();
        }
        let snapshots = list_snapshots(&conn, &chapter).unwrap();
        assert_eq!(snapshots.len() as i64, SNAPSHOTS_KEPT);
        assert_eq!(
            get_snapshot_content(&conn, &snapshots[0].id).unwrap(),
            "<p>version 28</p>"
        );
    }

    #[test]
    fn restore_brings_back_old_text_and_keeps_the_text_it_replaced() {
        let mut conn = open_in_memory().unwrap();
        let (_, chapter) = story_with_chapter(&mut conn);
        save_chapter_content(&mut conn, &chapter, "<p>old</p>", MINUTE).unwrap();
        save_chapter_content(&mut conn, &chapter, "<p>new</p>", 2 * MINUTE).unwrap();
        let old = list_snapshots(&conn, &chapter).unwrap().remove(0);

        // Inside the interval, so only the restore's own rule can save "new".
        let restored = restore_snapshot(&mut conn, &old.id, 3 * MINUTE).unwrap();

        assert_eq!(restored.content, "<p>old</p>");
        assert_eq!(restored.summary.word_count, 1);
        let kept: Vec<String> = list_snapshots(&conn, &chapter)
            .unwrap()
            .iter()
            .map(|s| get_snapshot_content(&conn, &s.id).unwrap())
            .collect();
        assert!(kept.contains(&"<p>new</p>".to_string()));
    }

    #[test]
    fn reorder_rewrites_positions_and_refuses_a_stale_list() {
        let mut conn = open_in_memory().unwrap();
        let (story, first) = story_with_chapter(&mut conn);
        let second = create_chapter(&mut conn, &story, "", 1).unwrap().summary;
        assert_eq!(second.title, "Chapter 2");

        let order = vec![second.id.clone(), first.clone()];
        let reordered = reorder_chapters(&mut conn, &story, &order, 2).unwrap();
        let ids: Vec<&str> = reordered.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, [second.id.as_str(), first.as_str()]);

        let stale = reorder_chapters(&mut conn, &story, &[first], 3);
        assert!(matches!(stale, Err(AppError::Validation(_))));
    }

    #[test]
    fn deleting_a_story_removes_everything_under_it() {
        let mut conn = open_in_memory().unwrap();
        let (story, chapter) = story_with_chapter(&mut conn);
        save_chapter_content(&mut conn, &chapter, "<p>a</p>", MINUTE).unwrap();
        save_chapter_content(&mut conn, &chapter, "<p>b</p>", 2 * MINUTE).unwrap();
        let character = Character {
            id: String::new(),
            story_id: story.clone(),
            name: "  Elena ".into(),
            age: Some(31),
            soul: String::new(),
            personality: String::new(),
            voice: String::new(),
            backstory: String::new(),
            affiliations: String::new(),
            notes: String::new(),
        };
        let saved = save_character(&conn, character, 3).unwrap();
        assert_eq!(saved.name, "Elena");

        delete_story(&conn, &story).unwrap();

        for table in ["chapters", "chapter_snapshots", "characters"] {
            let left: i64 = conn
                .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
                .unwrap();
            assert_eq!(left, 0, "{table} should be empty");
        }
    }

    #[test]
    fn oversized_and_missing_input_is_refused() {
        let mut conn = open_in_memory().unwrap();
        let long_title = "x".repeat(MAX_TITLE_CHARS + 1);
        assert!(matches!(
            create_story(&mut conn, &long_title, 0),
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            save_chapter_content(&mut conn, "missing", "<p>x</p>", 0),
            Err(AppError::NotFound("chapter"))
        ));
    }
}
