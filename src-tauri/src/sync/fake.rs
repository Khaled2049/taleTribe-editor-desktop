//! An in-memory website for tests of publishing and importing.

use std::sync::{Mutex, MutexGuard};

use rusqlite::Connection;
use serde_json::Value;

use super::publish::{Choice, Decisions, Mode, Outcome, Publisher, Summary};
use super::remote::{
    ChapterFields, ChapterText, EntityKind, Remote, RemoteChapter, RemoteEntity, RemoteStory,
    Reply, StoryFields, StoryListing,
};
use super::SyncError;
use crate::db::open_in_memory;
use crate::error::AppResult;
use crate::store::{self, word_count, Character};

pub const UID: &str = "writer-1";
pub const PNG: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 9];

pub struct SiteChapter {
    pub id: String,
    pub title: String,
    pub content: String,
    pub position: f64,
    pub revision: i64,
    pub released: bool,
}

/// An in-memory website that behaves like story-data where publishing cares:
/// revisions, the starter chapter, and positions that must stay unique.
#[derive(Default)]
pub struct Site {
    pub declined_agreement: bool,
    pub story: Option<RemoteStory>,
    pub chapters: Vec<SiteChapter>,
    pub entities: Vec<(EntityKind, RemoteEntity)>,
    pub images: Vec<String>,
    /// Uploaded image bytes by URL, for downloads.
    pub files: std::collections::HashMap<String, Vec<u8>>,
    pub writes: usize,
    /// Writes beyond this many fail, as a dropped connection would.
    pub fail_after: Option<usize>,
    ids: u32,
}

#[derive(Default)]
pub struct FakeRemote(Mutex<Site>);

impl FakeRemote {
    pub fn site(&self) -> MutexGuard<'_, Site> {
        self.0.lock().unwrap()
    }

    fn write(&self) -> Reply<MutexGuard<'_, Site>> {
        let mut site = self.site();
        if site.fail_after.is_some_and(|limit| site.writes >= limit) {
            return Err(SyncError::Network);
        }
        site.writes += 1;
        site.ids += 1;
        Ok(site)
    }

    pub fn titles(&self) -> Vec<String> {
        let mut site = self.site();
        site.chapters
            .sort_by(|a, b| a.position.total_cmp(&b.position));
        site.chapters.iter().map(|c| c.title.clone()).collect()
    }
}

fn seen(chapter: &SiteChapter) -> RemoteChapter {
    RemoteChapter {
        id: chapter.id.clone(),
        revision: chapter.revision,
        position: chapter.position,
        word_count: word_count(&chapter.content),
        published_at: chapter.released.then(|| "2026-01-01T00:00:00Z".to_string()),
    }
}

fn free(site: &Site, position: f64, except: &str) {
    let clash = site
        .chapters
        .iter()
        .any(|c| c.position == position && c.id != except);
    assert!(!clash, "two chapters at position {position}");
}

impl Remote for FakeRemote {
    async fn agreement_accepted(&self) -> Reply<bool> {
        Ok(!self.site().declined_agreement)
    }

    async fn get_story(&self, _id: &str) -> Reply<RemoteStory> {
        self.site().story.clone().ok_or(SyncError::NotFound)
    }

    async fn create_story(&self, fields: &StoryFields) -> Reply<RemoteStory> {
        let mut site = self.write()?;
        let story = RemoteStory {
            id: format!("story-{}", site.ids),
            owner_id: UID.into(),
            revision: 1,
            fields: fields.clone(),
        };
        site.story = Some(story.clone());
        site.chapters = vec![SiteChapter {
            id: "starter".into(),
            title: "Chapter 1".into(),
            content: String::new(),
            position: 0.0,
            revision: 1,
            released: false,
        }];
        Ok(story)
    }

    async fn update_story(
        &self,
        _id: &str,
        revision: i64,
        fields: &StoryFields,
    ) -> Reply<RemoteStory> {
        let mut site = self.write()?;
        let story = site.story.as_mut().ok_or(SyncError::NotFound)?;
        if story.revision != revision {
            return Err(SyncError::Conflict);
        }
        story.revision += 1;
        story.fields = fields.clone();
        Ok(story.clone())
    }

    async fn list_chapters(&self, _story: &str) -> Reply<Vec<RemoteChapter>> {
        Ok(self.site().chapters.iter().map(seen).collect())
    }

    async fn create_chapter(&self, _story: &str, fields: &ChapterFields) -> Reply<RemoteChapter> {
        let mut site = self.write()?;
        free(&site, fields.position, "");
        let chapter = SiteChapter {
            id: format!("chapter-{}", site.ids),
            title: fields.title.clone(),
            content: fields.content.clone(),
            position: fields.position,
            revision: 1,
            released: false,
        };
        let out = seen(&chapter);
        site.chapters.push(chapter);
        Ok(out)
    }

    async fn update_chapter(
        &self,
        _story: &str,
        id: &str,
        revision: i64,
        fields: &ChapterFields,
    ) -> Reply<RemoteChapter> {
        let mut site = self.write()?;
        free(&site, fields.position, id);
        let chapter = site
            .chapters
            .iter_mut()
            .find(|c| c.id == id)
            .ok_or(SyncError::NotFound)?;
        if chapter.revision != revision {
            return Err(SyncError::Conflict);
        }
        chapter.revision += 1;
        chapter.title = fields.title.clone();
        chapter.content = fields.content.clone();
        chapter.position = fields.position;
        Ok(seen(chapter))
    }

    async fn delete_chapter(&self, _story: &str, id: &str, revision: i64) -> Reply<()> {
        let mut site = self.write()?;
        let index = site
            .chapters
            .iter()
            .position(|c| c.id == id)
            .ok_or(SyncError::NotFound)?;
        if site.chapters[index].revision != revision {
            return Err(SyncError::Conflict);
        }
        site.chapters.remove(index);
        Ok(())
    }

    async fn release_chapter(&self, _story: &str, id: &str, revision: i64) -> Reply<()> {
        let mut site = self.write()?;
        let chapter = site
            .chapters
            .iter_mut()
            .find(|c| c.id == id)
            .ok_or(SyncError::NotFound)?;
        if chapter.revision != revision {
            return Err(SyncError::Conflict);
        }
        assert!(
            word_count(&chapter.content) > 0,
            "story-data refuses to release an empty chapter"
        );
        chapter.released = true;
        Ok(())
    }

    async fn list_entities(&self, _story: &str, kind: EntityKind) -> Reply<Vec<RemoteEntity>> {
        Ok(self
            .site()
            .entities
            .iter()
            .filter(|(k, _)| *k == kind)
            .map(|(_, e)| e.clone())
            .collect())
    }

    async fn create_entity(
        &self,
        _story: &str,
        kind: EntityKind,
        input: &Value,
    ) -> Reply<RemoteEntity> {
        let mut site = self.write()?;
        let entity = RemoteEntity {
            id: format!("entity-{}", site.ids),
            revision: 1,
            body: input.clone(),
        };
        site.entities.push((kind, entity.clone()));
        Ok(entity)
    }

    async fn update_entity(
        &self,
        _story: &str,
        _kind: EntityKind,
        id: &str,
        revision: i64,
        input: &Value,
    ) -> Reply<RemoteEntity> {
        let mut site = self.write()?;
        let (_, entity) = site
            .entities
            .iter_mut()
            .find(|(_, e)| e.id == id)
            .ok_or(SyncError::NotFound)?;
        if entity.revision != revision {
            return Err(SyncError::Conflict);
        }
        entity.revision += 1;
        entity.body = input.clone();
        Ok(entity.clone())
    }

    async fn delete_entity(
        &self,
        _story: &str,
        _kind: EntityKind,
        id: &str,
        _revision: i64,
    ) -> Reply<()> {
        self.write()?.entities.retain(|(_, e)| e.id != id);
        Ok(())
    }

    async fn list_stories(&self) -> Reply<Vec<StoryListing>> {
        let site = self.site();
        Ok(site
            .story
            .iter()
            .map(|story| StoryListing {
                id: story.id.clone(),
                title: story.fields.title.clone(),
                description: story.fields.description.clone(),
                published: story.fields.published,
                chapter_count: site.chapters.len() as i64,
                word_count: site.chapters.iter().map(|c| word_count(&c.content)).sum(),
                updated_at: "2026-01-01T00:00:00Z".into(),
            })
            .collect())
    }

    async fn read_chapters(&self, _story: &str) -> Reply<Vec<ChapterText>> {
        let mut site = self.site();
        site.chapters
            .sort_by(|a, b| a.position.total_cmp(&b.position));
        Ok(site
            .chapters
            .iter()
            .map(|c| ChapterText {
                id: c.id.clone(),
                title: c.title.clone(),
                content: c.content.clone(),
                position: c.position,
                revision: c.revision,
            })
            .collect())
    }

    fn hosts_image(&self, url: &str) -> bool {
        url.starts_with("https://img.example/")
    }

    async fn download_image(&self, url: &str) -> Reply<Vec<u8>> {
        self.site()
            .files
            .get(url)
            .cloned()
            .ok_or(SyncError::NotFound)
    }

    async fn upload_image(&self, story: &str, name: &str, bytes: Vec<u8>) -> Reply<String> {
        let mut site = self.write()?;
        site.images.push(name.to_string());
        // A query string, as real storage URLs have, so `&` escaping is exercised.
        let url = format!("https://img.example/{story}/{name}?alt=media&token=t");
        site.files.insert(url.clone(), bytes);
        Ok(url)
    }
}

/// A local story: three chapters (the first with an image, the last empty)
/// and one character.
pub struct Desk {
    pub db: Mutex<Connection>,
    pub assets: tempfile::TempDir,
    pub story: String,
    pub chapters: Vec<String>,
    pub character: Character,
    pub clock: i64,
}

impl Desk {
    pub fn new() -> Self {
        let assets = tempfile::tempdir().unwrap();
        let image = crate::assets::save_image(assets.path(), PNG).unwrap();
        let mut conn = open_in_memory().unwrap();
        let story = store::create_story(&mut conn, "Tale", 1).unwrap().id;
        let one = store::list_chapters(&conn, &story).unwrap().remove(0).id;
        store::rename_chapter(&conn, &one, "One", 2).unwrap();
        let text = format!("<p>first words</p><img src=\"tt-asset:{image}\">");
        store::save_chapter_content(&mut conn, &one, &text, 3).unwrap();
        let two = store::create_chapter(&mut conn, &story, "Two", 4)
            .unwrap()
            .summary
            .id;
        store::save_chapter_content(&mut conn, &two, "<p>second</p>", 5).unwrap();
        let three = store::create_chapter(&mut conn, &story, "Three", 6)
            .unwrap()
            .summary
            .id;
        let character = Character {
            id: String::new(),
            story_id: story.clone(),
            name: "Elena".into(),
            age: None,
            soul: String::new(),
            personality: String::new(),
            voice: String::new(),
            backstory: String::new(),
            affiliations: String::new(),
            notes: String::new(),
        };
        let character = store::save_character(&conn, character, 7).unwrap();
        Self {
            db: Mutex::new(conn),
            assets,
            story,
            chapters: vec![one, two, three],
            character,
            clock: 100,
        }
    }

    pub fn conn(&self) -> MutexGuard<'_, Connection> {
        self.db.lock().unwrap()
    }

    pub fn tick(&mut self) -> i64 {
        self.clock += 1;
        self.clock
    }

    pub fn publish(
        &self,
        site: &FakeRemote,
        mode: Mode,
        decisions: &[(&str, Choice)],
    ) -> AppResult<Outcome> {
        let decisions: Decisions = decisions
            .iter()
            .map(|(id, choice)| (id.to_string(), *choice))
            .collect();
        let publisher = Publisher {
            db: &self.db,
            assets_dir: self.assets.path(),
            remote: site,
            uid: UID,
            progress: &|_| {},
        };
        tauri::async_runtime::block_on(publisher.publish(&self.story, mode, &decisions))
    }

    pub fn done(&self, site: &FakeRemote, mode: Mode) -> Summary {
        match self.publish(site, mode, &[]).unwrap() {
            Outcome::Done { summary, .. } => summary,
            other => panic!("expected the publish to finish, got {other:?}"),
        }
    }
}
