//! What a publish would do, worked out from the local database alone, and the
//! reasons it cannot go ahead. Nothing here touches the network.

use std::path::Path;

use rusqlite::Connection;
use serde::Serialize;

use crate::error::{AppError, AppResult};

// story-data's ceilings (`internal/store`), checked here so a publish is not
// rejected half-way through.
pub const CHAPTER_WORD_LIMIT: i64 = 5_000;
pub const STORY_CHAPTER_LIMIT: usize = 50;
const ENTITY_LIMIT: i64 = 200;
const CHAPTER_IMAGE_LIMIT: usize = 5;

const ASSET_PREFIX: &str = "tt-asset:";

#[derive(Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    pub new: i64,
    pub changed: i64,
    pub deleted: i64,
}

impl Counts {
    fn total(&self) -> i64 {
        self.new + self.changed + self.deleted
    }
}

#[derive(Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishPlan {
    /// The story is already on the website; publishing updates it.
    pub linked: bool,
    pub story_changed: bool,
    pub chapters: Counts,
    pub characters: Counts,
    pub places: Counts,
    pub images_to_upload: usize,
    /// Reasons the website would refuse this story. Publishing is blocked until fixed.
    pub problems: Vec<String>,
    pub nothing_to_send: bool,
}

/// The local images a chapter's HTML refers to.
pub fn asset_refs(html: &str) -> Vec<&str> {
    html.match_indices(ASSET_PREFIX)
        .map(|(at, _)| {
            let rest = &html[at + ASSET_PREFIX.len()..];
            let end = rest
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '.'))
                .unwrap_or(rest.len());
            &rest[..end]
        })
        .filter(|name| !name.is_empty())
        .collect()
}

/// Chapter HTML as the website stores it: each local image reference replaced
/// by the URL `url_for` gives. A reference with no URL is left as it is.
pub fn with_hosted_images(html: &str, url_for: impl Fn(&str) -> Option<String>) -> String {
    let mut out = html.to_string();
    for name in asset_refs(html) {
        if let Some(url) = url_for(name) {
            // The URL lands inside an HTML attribute, where `&` is written `&amp;`.
            out = out.replace(&format!("{ASSET_PREFIX}{name}"), &url.replace('&', "&amp;"));
        }
    }
    out
}

fn counts(conn: &Connection, table: &str, kind: &str, story_id: &str) -> AppResult<Counts> {
    let (new, changed): (i64, i64) = conn.query_row(
        &format!(
            "SELECT coalesce(sum(remote_id IS NULL), 0),
                    coalesce(sum(remote_id IS NOT NULL AND dirty = 1), 0)
             FROM {table} WHERE story_id = ?1"
        ),
        [story_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let deleted = conn.query_row(
        "SELECT count(*) FROM tombstones WHERE story_id = ?1 AND kind = ?2",
        [story_id, kind],
        |row| row.get(0),
    )?;
    Ok(Counts {
        new,
        changed,
        deleted,
    })
}

pub fn preview(conn: &Connection, assets_dir: &Path, story_id: &str) -> AppResult<PublishPlan> {
    let (remote_id, dirty): (Option<String>, bool) = conn
        .query_row(
            "SELECT remote_id, dirty FROM stories WHERE id = ?1",
            [story_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|_| AppError::NotFound("story"))?;

    let mut plan = PublishPlan {
        linked: remote_id.is_some(),
        story_changed: remote_id.is_none() || dirty,
        chapters: counts(conn, "chapters", "chapter", story_id)?,
        characters: counts(conn, "characters", "character", story_id)?,
        places: counts(conn, "places", "place", story_id)?,
        ..PublishPlan::default()
    };

    let mut stmt = conn.prepare(
        "SELECT title, content, word_count FROM chapters WHERE story_id = ?1 ORDER BY position, created_at",
    )?;
    let chapters: Vec<(String, String, i64)> = stmt
        .query_map([story_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?
        .collect::<Result<_, _>>()?;
    if chapters.len() > STORY_CHAPTER_LIMIT {
        plan.problems.push(format!(
            "This story has {} chapters. The website allows {STORY_CHAPTER_LIMIT}.",
            chapters.len()
        ));
    }
    let mut pending = std::collections::BTreeSet::new();
    for (index, (title, content, words)) in chapters.iter().enumerate() {
        let name = format!("Chapter {} \"{title}\"", index + 1);
        if *words > CHAPTER_WORD_LIMIT {
            plan.problems.push(format!(
                "{name} has {words} words. The website allows {CHAPTER_WORD_LIMIT} per chapter; split it into two."
            ));
        }
        let images = asset_refs(content);
        if images.len() > CHAPTER_IMAGE_LIMIT {
            plan.problems.push(format!(
                "{name} has {} images. The website allows {CHAPTER_IMAGE_LIMIT} per chapter.",
                images.len()
            ));
        }
        for image in images {
            if !assets_dir.join(image).is_file() {
                plan.problems.push(format!("{name} uses an image that is missing from this computer. Remove it from the chapter."));
            } else {
                pending.insert(image.to_string());
            }
        }
    }
    for (table, noun) in [("characters", "characters"), ("places", "places")] {
        let total: i64 = conn.query_row(
            &format!("SELECT count(*) FROM {table} WHERE story_id = ?1"),
            [story_id],
            |row| row.get(0),
        )?;
        if total > ENTITY_LIMIT {
            plan.problems.push(format!(
                "This story has {total} {noun}. The website allows {ENTITY_LIMIT}."
            ));
        }
    }

    if let Some(remote_id) = &remote_id {
        let mut stmt =
            conn.prepare("SELECT asset FROM asset_uploads WHERE remote_story_id = ?1")?;
        for uploaded in stmt.query_map([remote_id], |row| row.get::<_, String>(0))? {
            pending.remove(&uploaded?);
        }
    }
    plan.images_to_upload = pending.len();
    plan.nothing_to_send = !plan.story_changed
        && plan.chapters.total() + plan.characters.total() + plan.places.total() == 0;
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;
    use crate::store;

    #[test]
    fn image_references_are_found_and_swapped_for_hosted_urls() {
        let html =
            r#"<p>a</p><img src="tt-asset:abc123.png" alt="x"><img src="tt-asset:def.webp">"#;
        assert_eq!(asset_refs(html), ["abc123.png", "def.webp"]);
        let hosted = with_hosted_images(html, |name| {
            (name == "abc123.png").then(|| "https://cdn/x.png?alt=media&token=t".into())
        });
        assert_eq!(
            hosted,
            r#"<p>a</p><img src="https://cdn/x.png?alt=media&amp;token=t" alt="x"><img src="tt-asset:def.webp">"#
        );
    }

    #[test]
    fn a_new_story_is_all_new_and_limits_block_it() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = open_in_memory().unwrap();
        let story = store::create_story(&mut conn, "Draft", 0).unwrap();
        let chapter = store::list_chapters(&conn, &story.id).unwrap().remove(0).id;

        let plan = preview(&conn, dir.path(), &story.id).unwrap();
        assert!(!plan.linked && plan.story_changed && !plan.nothing_to_send);
        assert_eq!(
            plan.chapters,
            Counts {
                new: 1,
                changed: 0,
                deleted: 0
            }
        );
        assert!(plan.problems.is_empty());

        let long = format!(
            "<p>{}</p><img src=\"tt-asset:gone.png\">",
            "word ".repeat(5_001)
        );
        store::save_chapter_content(&mut conn, &chapter, &long, 1).unwrap();
        let plan = preview(&conn, dir.path(), &story.id).unwrap();
        assert_eq!(plan.problems.len(), 2, "{:?}", plan.problems);
        assert!(plan.problems[0].contains("5000 per chapter"));
        assert!(plan.problems[1].contains("missing from this computer"));
    }
}
