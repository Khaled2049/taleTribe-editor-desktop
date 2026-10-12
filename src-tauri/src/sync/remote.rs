//! The website as the publisher sees it. `HttpRemote` is the real thing; tests
//! drive the publisher with an in-memory stand-in.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::SyncError;

pub type Reply<T> = Result<T, SyncError>;

/// story-data's `StoryInput`. An update replaces all of it, so fields this app
/// does not edit are read from the website and sent back unchanged.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct StoryFields {
    pub title: String,
    pub description: String,
    pub author_name: String,
    pub category: String,
    pub target_audience: String,
    pub language: String,
    pub copyright: String,
    pub cover_image_url: String,
    pub thumbnail_url: String,
    pub tags: Vec<String>,
    pub paragraph_style: String,
    pub published: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteStory {
    pub id: String,
    pub owner_id: String,
    pub revision: i64,
    #[serde(flatten)]
    pub fields: StoryFields,
}

/// story-data's `ChapterInput`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChapterFields {
    pub title: String,
    pub content: String,
    pub position: f64,
}

/// A chapter as the website holds it, without its text.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteChapter {
    pub id: String,
    pub revision: i64,
    pub position: f64,
    #[serde(default)]
    pub word_count: i64,
    #[serde(default)]
    pub published_at: Option<String>,
}

impl RemoteChapter {
    pub fn is_released(&self) -> bool {
        self.published_at.is_some()
    }
}

/// One of the writer's website stories, as listed for import.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoryListing {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub published: bool,
    #[serde(default)]
    pub chapter_count: i64,
    #[serde(default)]
    pub word_count: i64,
    #[serde(default)]
    pub updated_at: String,
}

/// A website chapter with its text.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ChapterText {
    pub id: String,
    pub title: String,
    pub content: String,
    pub position: f64,
    pub revision: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntityKind {
    Character,
    Place,
}

impl EntityKind {
    pub fn path(self) -> &'static str {
        match self {
            Self::Character => "characters",
            Self::Place => "places",
        }
    }

    pub fn table(self) -> &'static str {
        self.path()
    }

    pub fn tombstone(self) -> &'static str {
        match self {
            Self::Character => "character",
            Self::Place => "place",
        }
    }
}

/// A character or place on the website. `body` is the whole record, kept so
/// the parts this app does not edit (art, relationships) can be sent back.
#[derive(Debug, Clone, PartialEq)]
pub struct RemoteEntity {
    pub id: String,
    pub revision: i64,
    pub body: Value,
}

impl RemoteEntity {
    pub fn from_json(body: Value) -> Option<Self> {
        Some(Self {
            id: body["id"].as_str()?.to_string(),
            revision: body["revision"].as_i64()?,
            body,
        })
    }
}

#[allow(async_fn_in_trait)]
pub trait Remote {
    async fn agreement_accepted(&self) -> Reply<bool>;

    async fn get_story(&self, id: &str) -> Reply<RemoteStory>;
    async fn create_story(&self, fields: &StoryFields) -> Reply<RemoteStory>;
    async fn update_story(
        &self,
        id: &str,
        revision: i64,
        fields: &StoryFields,
    ) -> Reply<RemoteStory>;

    async fn list_chapters(&self, story: &str) -> Reply<Vec<RemoteChapter>>;
    async fn create_chapter(&self, story: &str, fields: &ChapterFields) -> Reply<RemoteChapter>;
    async fn update_chapter(
        &self,
        story: &str,
        id: &str,
        revision: i64,
        fields: &ChapterFields,
    ) -> Reply<RemoteChapter>;
    async fn delete_chapter(&self, story: &str, id: &str, revision: i64) -> Reply<()>;
    /// Makes a chapter visible to readers.
    async fn release_chapter(&self, story: &str, id: &str, revision: i64) -> Reply<()>;

    async fn list_entities(&self, story: &str, kind: EntityKind) -> Reply<Vec<RemoteEntity>>;
    async fn create_entity(
        &self,
        story: &str,
        kind: EntityKind,
        input: &Value,
    ) -> Reply<RemoteEntity>;
    async fn update_entity(
        &self,
        story: &str,
        kind: EntityKind,
        id: &str,
        revision: i64,
        input: &Value,
    ) -> Reply<RemoteEntity>;
    async fn delete_entity(
        &self,
        story: &str,
        kind: EntityKind,
        id: &str,
        revision: i64,
    ) -> Reply<()>;

    /// The signed-in writer's own stories.
    async fn list_stories(&self) -> Reply<Vec<StoryListing>>;
    async fn read_chapters(&self, story: &str) -> Reply<Vec<ChapterText>>;
    /// Whether `url` is an image the website itself stores. Nothing else is downloaded.
    fn hosts_image(&self, url: &str) -> bool;
    async fn download_image(&self, url: &str) -> Reply<Vec<u8>>;

    /// Stores a chapter image for this story and returns its public URL.
    async fn upload_image(&self, story: &str, name: &str, bytes: Vec<u8>) -> Reply<String>;
}
