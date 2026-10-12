//! The whole surface the webview can call. Each command only locks the
//! database, stamps the time and delegates to `store` or `assets`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use futures_util::future::{AbortHandle, Abortable};
use rusqlite::Connection;
use serde::Serialize;
use tauri::ipc::{Channel, InvokeBody, Request};
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::ai::providers::{self, ModelInfo, Target};
use crate::ai::settings::{self, AiSettings};
use crate::ai::{context, prompts, AiError, Provider, Task};
use crate::assets;
use crate::db::now_ms;
use crate::error::{AppError, AppResult};
use crate::secrets::SecretStore;
use crate::store::{self, Chapter, ChapterSummary, Character, Place, Snapshot, Story};
use crate::sync::account::{self, Account, Session, REFRESH_TOKEN};
use crate::sync::config::Config;
use crate::sync::http::HttpRemote;
use crate::sync::import::{Existing, Imported, Importer};
use crate::sync::plan::{self, PublishPlan};
use crate::sync::publish::{Decisions, Mode, Outcome, Publisher};
use crate::sync::remote::{Remote, StoryListing};
use crate::sync::SyncError;

pub struct AppState {
    pub db: Mutex<Connection>,
    pub assets_dir: PathBuf,
    pub secrets: Box<dyn SecretStore>,
    pub http: reqwest::Client,
    /// Requests in flight, by the id the webview gave them, so they can be cancelled.
    pub generations: Mutex<HashMap<String, AbortHandle>>,
    pub config: Config,
    /// The signed-in account's current token. Memory only; gone when the app closes.
    pub session: Mutex<Option<Session>>,
}

impl AppState {
    // A panic mid-command rolls its transaction back, so the connection is still usable.
    fn db(&self) -> MutexGuard<'_, Connection> {
        self.db
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

type Ctx<'a> = State<'a, AppState>;

#[tauri::command(async)]
pub fn list_stories(state: Ctx<'_>) -> AppResult<Vec<Story>> {
    store::list_stories(&state.db())
}

#[tauri::command(async)]
pub fn create_story(state: Ctx<'_>, title: String) -> AppResult<Story> {
    store::create_story(&mut state.db(), &title, now_ms())
}

#[tauri::command(async)]
pub fn update_story(
    state: Ctx<'_>,
    id: String,
    title: String,
    description: String,
) -> AppResult<Story> {
    store::update_story(&state.db(), &id, &title, &description, now_ms())
}

#[tauri::command(async)]
pub fn delete_story(state: Ctx<'_>, id: String) -> AppResult<()> {
    store::delete_story(&state.db(), &id)
}

#[tauri::command(async)]
pub fn list_chapters(state: Ctx<'_>, story_id: String) -> AppResult<Vec<ChapterSummary>> {
    store::list_chapters(&state.db(), &story_id)
}

#[tauri::command(async)]
pub fn get_chapter(state: Ctx<'_>, id: String) -> AppResult<Chapter> {
    store::get_chapter(&state.db(), &id)
}

#[tauri::command(async)]
pub fn create_chapter(state: Ctx<'_>, story_id: String, title: String) -> AppResult<Chapter> {
    store::create_chapter(&mut state.db(), &story_id, &title, now_ms())
}

#[tauri::command(async)]
pub fn rename_chapter(state: Ctx<'_>, id: String, title: String) -> AppResult<Chapter> {
    store::rename_chapter(&state.db(), &id, &title, now_ms())
}

#[tauri::command(async)]
pub fn delete_chapter(state: Ctx<'_>, id: String) -> AppResult<()> {
    store::delete_chapter(&state.db(), &id)
}

#[tauri::command(async)]
pub fn reorder_chapters(
    state: Ctx<'_>,
    story_id: String,
    ordered_ids: Vec<String>,
) -> AppResult<Vec<ChapterSummary>> {
    store::reorder_chapters(&mut state.db(), &story_id, &ordered_ids, now_ms())
}

#[tauri::command(async)]
pub fn save_chapter_content(
    state: Ctx<'_>,
    id: String,
    content: String,
) -> AppResult<ChapterSummary> {
    store::save_chapter_content(&mut state.db(), &id, &content, now_ms())
}

#[tauri::command(async)]
pub fn list_snapshots(state: Ctx<'_>, chapter_id: String) -> AppResult<Vec<Snapshot>> {
    store::list_snapshots(&state.db(), &chapter_id)
}

#[tauri::command(async)]
pub fn get_snapshot_content(state: Ctx<'_>, id: String) -> AppResult<String> {
    store::get_snapshot_content(&state.db(), &id)
}

#[tauri::command(async)]
pub fn restore_snapshot(state: Ctx<'_>, id: String) -> AppResult<Chapter> {
    store::restore_snapshot(&mut state.db(), &id, now_ms())
}

#[tauri::command(async)]
pub fn list_characters(state: Ctx<'_>, story_id: String) -> AppResult<Vec<Character>> {
    store::list_characters(&state.db(), &story_id)
}

#[tauri::command(async)]
pub fn save_character(state: Ctx<'_>, character: Character) -> AppResult<Character> {
    store::save_character(&state.db(), character, now_ms())
}

#[tauri::command(async)]
pub fn delete_character(state: Ctx<'_>, id: String) -> AppResult<()> {
    store::delete_character(&state.db(), &id)
}

#[tauri::command(async)]
pub fn list_places(state: Ctx<'_>, story_id: String) -> AppResult<Vec<Place>> {
    store::list_places(&state.db(), &story_id)
}

#[tauri::command(async)]
pub fn save_place(state: Ctx<'_>, place: Place) -> AppResult<Place> {
    store::save_place(&state.db(), place, now_ms())
}

#[tauri::command(async)]
pub fn delete_place(state: Ctx<'_>, id: String) -> AppResult<()> {
    store::delete_place(&state.db(), &id)
}

/// Takes the image as the raw request body, avoiding a JSON array of bytes.
#[tauri::command(async)]
pub fn import_image(state: Ctx<'_>, request: Request<'_>) -> AppResult<String> {
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err(AppError::validation("Expected image bytes."));
    };
    assets::save_image(&state.assets_dir, bytes)
}

// ── AI ───────────────────────────────────────────────────────────────────────

/// Settings as the webview sees them: whether a key is saved, never the key.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiStatus {
    settings: AiSettings,
    has_anthropic_key: bool,
    has_openai_key: bool,
}

fn ai_status(state: &AppState, settings: AiSettings) -> AppResult<AiStatus> {
    Ok(AiStatus {
        settings,
        has_anthropic_key: state
            .secrets
            .get(&Provider::Anthropic.key_name())?
            .is_some(),
        has_openai_key: state.secrets.get(&Provider::Openai.key_name())?.is_some(),
    })
}

fn target(state: &AppState, settings: &AiSettings, provider: Provider) -> AppResult<Target> {
    Ok(Target {
        provider,
        ollama_url: settings.ollama_url.clone(),
        model: settings.model(provider).clone(),
        key: match provider.needs_key() {
            true => Some(
                state
                    .secrets
                    .get(&provider.key_name())?
                    .ok_or(AiError::MissingKey)?,
            ),
            false => None,
        },
    })
}

#[tauri::command(async)]
pub fn get_ai_settings(state: Ctx<'_>) -> AppResult<AiStatus> {
    let settings = settings::load(&state.db())?;
    ai_status(&state, settings)
}

#[tauri::command(async)]
pub fn save_ai_settings(state: Ctx<'_>, settings: AiSettings) -> AppResult<AiStatus> {
    let saved = settings::save(&state.db(), settings)?;
    ai_status(&state, saved)
}

#[tauri::command(async)]
pub fn set_api_key(state: Ctx<'_>, provider: Provider, key: String) -> AppResult<()> {
    let key = key.trim();
    if !provider.needs_key() || key.is_empty() || key.len() > 500 {
        return Err(AppError::validation("Enter the API key for this provider."));
    }
    Ok(state.secrets.set(&provider.key_name(), key)?)
}

#[tauri::command(async)]
pub fn delete_api_key(state: Ctx<'_>, provider: Provider) -> AppResult<()> {
    Ok(state.secrets.delete(&provider.key_name())?)
}

/// `ollama_url` lets Settings list models at an address that is not saved yet.
#[tauri::command]
pub async fn list_models(
    state: Ctx<'_>,
    provider: Provider,
    ollama_url: Option<String>,
) -> AppResult<Vec<ModelInfo>> {
    let mut settings = settings::load(&state.db())?;
    if let Some(url) = ollama_url {
        settings.ollama_url = settings::normalize_ollama_url(&url)?;
    }
    let target = target(&state, &settings, provider)?;
    Ok(providers::list_models(&state.http, &target).await?)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Generated {
    text: String,
    /// Set for next-line requests, where the answer is a list.
    suggestions: Option<Vec<String>>,
}

/// Runs one AI request. Pieces of the answer are sent over `on_text` as they
/// arrive; the whole answer is the return value.
#[tauri::command]
pub async fn generate(
    state: Ctx<'_>,
    request_id: String,
    task: Task,
    on_text: Channel<String>,
) -> AppResult<Generated> {
    // Everything read from the database happens before the first await.
    let (target, prompt) = {
        let conn = state.db();
        let settings = settings::load(&conn)?;
        let provider = settings.provider.ok_or(AiError::NotConfigured)?;
        let budget = settings.context_budget.chars();
        let prompt = match &task {
            Task::NextLines {
                story_id,
                chapter_id,
                before,
                after,
            } => {
                let story = context::load(&conn, story_id, Some(chapter_id))?;
                prompts::next_lines(&story, budget, before, after)
            }
            Task::Enhance {
                story_id,
                action,
                text,
            } => prompts::enhance(&context::load(&conn, story_id, None)?, *action, text),
            Task::Brainstorm {
                story_id,
                kind,
                prompt,
            } => {
                let story = context::load(&conn, story_id, None)?;
                prompts::brainstorm(&story, budget, *kind, prompt)
            }
        };
        (target(&state, &settings, provider)?, prompt)
    };

    let (handle, registration) = AbortHandle::new_pair();
    let generations = || state.generations.lock().unwrap_or_else(|p| p.into_inner());
    generations().insert(request_id.clone(), handle);
    let streaming = providers::stream(&state.http, &target, &prompt, |text| {
        // A closed channel only means the panel was closed; the answer still returns.
        let _ = on_text.send(text.to_string());
    });
    let outcome = Abortable::new(streaming, registration).await;
    generations().remove(&request_id);

    let text = outcome.map_err(|_| AiError::Cancelled)??;
    let suggestions = match task {
        Task::NextLines { .. } => {
            Some(prompts::parse_suggestions(&text).ok_or(AiError::BadAnswer)?)
        }
        _ => None,
    };
    Ok(Generated { text, suggestions })
}

#[tauri::command(async)]
pub fn cancel_generation(state: Ctx<'_>, request_id: String) {
    let handle = state
        .generations
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .remove(&request_id);
    if let Some(handle) = handle {
        handle.abort();
    }
}

// ── Website account and publishing ───────────────────────────────────────────

fn remember(state: &AppState, signed: account::SignedIn) -> AppResult<Session> {
    state
        .secrets
        .set(REFRESH_TOKEN, signed.refresh_token.expose())?;
    account::save_account(&state.db(), &signed.session.account)?;
    *state.session.lock().unwrap_or_else(|p| p.into_inner()) = Some(signed.session.clone());
    Ok(signed.session)
}

fn forget(state: &AppState) -> AppResult<()> {
    *state.session.lock().unwrap_or_else(|p| p.into_inner()) = None;
    state.secrets.delete(REFRESH_TOKEN)?;
    account::clear_account(&state.db())
}

/// A token good for the next request, renewed from the keychain when needed.
async fn session(state: &AppState) -> AppResult<Session> {
    let cached = state
        .session
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone();
    if let Some(session) = cached.filter(Session::is_fresh) {
        return Ok(session);
    }
    let saved = account::saved_account(&state.db())?.ok_or(SyncError::SignInNeeded)?;
    let token = state
        .secrets
        .get(REFRESH_TOKEN)?
        .ok_or(SyncError::SignInNeeded)?;
    match account::refresh(&state.http, &state.config, &saved, &token).await {
        Ok(signed) => remember(state, signed),
        Err(SyncError::SignInNeeded) => {
            // Signed out elsewhere, or the account is gone: stop showing it as signed in.
            forget(state)?;
            Err(SyncError::SignInNeeded.into())
        }
        Err(error) => Err(error.into()),
    }
}

#[tauri::command(async)]
pub fn get_account(state: Ctx<'_>) -> AppResult<Option<Account>> {
    account::saved_account(&state.db())
}

#[tauri::command]
pub async fn sign_in(state: Ctx<'_>, email: String, password: String) -> AppResult<Account> {
    let signed = account::sign_in(&state.http, &state.config, email.trim(), &password).await?;
    Ok(remember(&state, signed)?.account)
}

#[tauri::command(async)]
pub fn sign_out(state: Ctx<'_>) -> AppResult<()> {
    forget(&state)
}

/// What publishing this story would send, from local data only.
#[tauri::command(async)]
pub fn publish_preview(state: Ctx<'_>, story_id: String) -> AppResult<PublishPlan> {
    plan::preview(&state.db(), &state.assets_dir, &story_id)
}

#[tauri::command]
pub async fn publish(
    state: Ctx<'_>,
    story_id: String,
    mode: Mode,
    decisions: Decisions,
    on_progress: Channel<String>,
) -> AppResult<Outcome> {
    let session = session(&state).await?;
    let progress = |step: String| {
        let _ = on_progress.send(step);
    };
    let remote = HttpRemote {
        http: &state.http,
        config: &state.config,
        session: session.clone(),
        on_wait: Box::new(|seconds| {
            progress(format!(
                "The website asked us to slow down. Continuing in {seconds} seconds"
            ))
        }),
    };
    let publisher = Publisher {
        db: &state.db,
        assets_dir: &state.assets_dir,
        remote: &remote,
        uid: &session.account.uid,
        progress: &progress,
    };
    publisher.publish(&story_id, mode, &decisions).await
}

/// A website story offered for import.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebsiteStory {
    #[serde(flatten)]
    listing: StoryListing,
    /// The local story already holding it, if there is one.
    local_id: Option<String>,
}

/// The signed-in writer's stories on the website.
#[tauri::command]
pub async fn list_website_stories(state: Ctx<'_>) -> AppResult<Vec<WebsiteStory>> {
    let session = session(&state).await?;
    let remote = HttpRemote {
        http: &state.http,
        config: &state.config,
        session,
        on_wait: Box::new(|_| {}),
    };
    let listed = remote.list_stories().await?;
    let conn = state.db();
    let mut local = conn.prepare("SELECT id FROM stories WHERE remote_id = ?1")?;
    listed
        .into_iter()
        .map(|listing| {
            let local_id = local.query_row([&listing.id], |row| row.get(0)).ok();
            Ok(WebsiteStory { listing, local_id })
        })
        .collect()
}

/// Copies a website story onto this computer. `existing` answers
/// `Imported::AlreadyHere` from an earlier attempt.
#[tauri::command]
pub async fn import_story(
    state: Ctx<'_>,
    remote_id: String,
    existing: Option<Existing>,
    on_progress: Channel<String>,
) -> AppResult<Imported> {
    let session = session(&state).await?;
    let progress = |step: String| {
        let _ = on_progress.send(step);
    };
    let remote = HttpRemote {
        http: &state.http,
        config: &state.config,
        session: session.clone(),
        on_wait: Box::new(|_| {}),
    };
    let importer = Importer {
        db: &state.db,
        assets_dir: &state.assets_dir,
        remote: &remote,
        uid: &session.account.uid,
        progress: &progress,
    };
    importer.import(&remote_id, existing, now_ms()).await
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WebPage {
    /// Where a writer accepts the writer agreement.
    Agreement,
    /// This story's public page. Only exists once the story is public.
    Story,
    /// This story in the website's editor, which is where a draft can be seen.
    Editor,
}

/// Opens a website page in the writer's browser. The address is built here from
/// the build's settings and the database, never from text the webview supplies.
#[tauri::command(async)]
pub fn open_website(
    app: AppHandle,
    state: Ctx<'_>,
    page: WebPage,
    story_id: Option<String>,
) -> AppResult<()> {
    let url = match page {
        WebPage::Agreement => format!("{}/user-stories", state.config.web_url),
        WebPage::Story | WebPage::Editor => {
            let remote_id: Option<String> = state.db().query_row(
                "SELECT remote_id FROM stories WHERE id = ?1",
                [story_id.unwrap_or_default()],
                |row| row.get(0),
            )?;
            let remote_id = remote_id.ok_or(AppError::NotFound("story"))?;
            let section = if matches!(page, WebPage::Story) {
                "story"
            } else {
                "create"
            };
            format!("{}/{section}/{remote_id}", state.config.web_url)
        }
    };
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|_| AppError::validation("Couldn't open your browser."))
}
