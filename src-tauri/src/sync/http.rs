//! The website over HTTP: story-data for stories, Firebase Storage for images,
//! and the Cloud Function that meters image uploads.

use std::time::Duration;

use reqwest::{header, Method, StatusCode};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};

use super::account::Session;
use super::config::Config;
use super::remote::{
    ChapterFields, ChapterText, EntityKind, Remote, RemoteChapter, RemoteEntity, RemoteStory,
    Reply, StoryFields, StoryListing,
};
use super::SyncError;

/// How many times a rate-limited request is retried, and the longest single wait.
const RATE_LIMIT_TRIES: u32 = 6;
const MAX_WAIT: Duration = Duration::from_secs(65);

pub struct HttpRemote<'a> {
    pub http: &'a reqwest::Client,
    pub config: &'a Config,
    pub session: Session,
    /// Told when a request has to wait, so the writer sees why nothing moves.
    pub on_wait: Box<dyn Fn(u64) + Send + Sync + 'a>,
}

/// story-data answers failures with `{"error": "..."}`.
fn message(body: &str) -> String {
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|json| json["error"].as_str().map(str::to_string))
        .unwrap_or_else(|| "unexpected response".into())
}

fn failure(status: StatusCode, body: &str) -> SyncError {
    match status.as_u16() {
        401 => SyncError::SignInNeeded,
        404 => SyncError::NotFound,
        409 => SyncError::Conflict,
        // 403 carries the writer-agreement notice; 400 and 422 a limit or validation message.
        400 | 403 | 422 => SyncError::Rejected(message(body)),
        429 => SyncError::RateLimited,
        _ => SyncError::Server(format!("HTTP {}", status.as_u16())),
    }
}

fn retry_after(response: &reqwest::Response) -> Duration {
    let seconds = response
        .headers()
        .get(header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse().ok())
        .unwrap_or(5);
    Duration::from_secs(seconds).min(MAX_WAIT)
}

fn encode(path: &str) -> String {
    path.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn mime(name: &str) -> &'static str {
    match name.rsplit('.').next() {
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        _ => "image/jpeg",
    }
}

/// Firebase Storage's upload format: a JSON part describing the file, then the file.
fn multipart(boundary: &str, path: &str, content_type: &str, bytes: &[u8]) -> Vec<u8> {
    let metadata = json!({
        "name": path,
        "contentType": content_type,
        "cacheControl": "public, max-age=31536000",
    });
    let mut body = format!(
        "--{boundary}\r\nContent-Type: application/json; charset=utf-8\r\n\r\n{metadata}\r\n--{boundary}\r\nContent-Type: {content_type}\r\n\r\n"
    )
    .into_bytes();
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--").as_bytes());
    body
}

impl HttpRemote<'_> {
    /// One story-data call. A stale `revision` comes back as `Conflict`.
    async fn call<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        revision: Option<i64>,
        body: Option<&Value>,
    ) -> Reply<T> {
        let text = self.raw(method, path, revision, body).await?;
        // A 204 has no body; `null` lets callers ask for `()`.
        let text = if text.is_empty() { "null" } else { &text };
        serde_json::from_str(text).map_err(|_| SyncError::Server("unexpected response".into()))
    }

    async fn raw(
        &self,
        method: Method,
        path: &str,
        revision: Option<i64>,
        body: Option<&Value>,
    ) -> Reply<String> {
        for attempt in 1..=RATE_LIMIT_TRIES {
            let mut request = self.http.request(
                method.clone(),
                format!("{}{path}", self.config.story_data_url),
            );
            request = if self.config.emulators {
                // The local story-data runs with AUTH_MODE=dev and takes the uid directly.
                request.header("X-User-ID", &self.session.account.uid)
            } else {
                request.bearer_auth(self.session.token.expose())
            };
            if let Some(revision) = revision {
                request = request.header(header::IF_MATCH, revision);
            }
            if let Some(body) = body {
                request = request.json(body);
            }
            let response = request.send().await.map_err(|_| SyncError::Network)?;
            let status = response.status();
            if status == StatusCode::TOO_MANY_REQUESTS && attempt < RATE_LIMIT_TRIES {
                let wait = retry_after(&response);
                (self.on_wait)(wait.as_secs());
                tokio::time::sleep(wait).await;
                continue;
            }
            let text = response.text().await.map_err(|_| SyncError::Network)?;
            return if status.is_success() {
                Ok(text)
            } else {
                Err(failure(status, &text))
            };
        }
        Err(SyncError::RateLimited)
    }

    fn entity(body: Value) -> Reply<RemoteEntity> {
        RemoteEntity::from_json(body).ok_or_else(|| SyncError::Server("unexpected response".into()))
    }
}

impl Remote for HttpRemote<'_> {
    async fn agreement_accepted(&self) -> Reply<bool> {
        let agreement: Value = self
            .call(Method::GET, "/v1/me/writer-agreement", None, None)
            .await?;
        Ok(agreement["accepted"] == true)
    }

    async fn get_story(&self, id: &str) -> Reply<RemoteStory> {
        self.call(Method::GET, &format!("/v1/stories/{id}"), None, None)
            .await
    }

    async fn create_story(&self, fields: &StoryFields) -> Reply<RemoteStory> {
        self.call(Method::POST, "/v1/stories", None, Some(&json!(fields)))
            .await
    }

    async fn update_story(
        &self,
        id: &str,
        revision: i64,
        fields: &StoryFields,
    ) -> Reply<RemoteStory> {
        self.call(
            Method::PATCH,
            &format!("/v1/stories/{id}"),
            Some(revision),
            Some(&json!(fields)),
        )
        .await
    }

    async fn list_chapters(&self, story: &str) -> Reply<Vec<RemoteChapter>> {
        self.call(
            Method::GET,
            &format!("/v1/stories/{story}/chapters?content=false"),
            None,
            None,
        )
        .await
    }

    async fn create_chapter(&self, story: &str, fields: &ChapterFields) -> Reply<RemoteChapter> {
        self.call(
            Method::POST,
            &format!("/v1/stories/{story}/chapters"),
            None,
            Some(&json!(fields)),
        )
        .await
    }

    async fn update_chapter(
        &self,
        story: &str,
        id: &str,
        revision: i64,
        fields: &ChapterFields,
    ) -> Reply<RemoteChapter> {
        self.call(
            Method::PATCH,
            &format!("/v1/stories/{story}/chapters/{id}"),
            Some(revision),
            Some(&json!(fields)),
        )
        .await
    }

    async fn delete_chapter(&self, story: &str, id: &str, revision: i64) -> Reply<()> {
        self.call(
            Method::DELETE,
            &format!("/v1/stories/{story}/chapters/{id}"),
            Some(revision),
            None,
        )
        .await
    }

    async fn release_chapter(&self, story: &str, id: &str, revision: i64) -> Reply<()> {
        let path = format!("/v1/stories/{story}/chapters/{id}/publication");
        self.call::<Value>(
            Method::PUT,
            &path,
            Some(revision),
            Some(&json!({ "published": true })),
        )
        .await?;
        Ok(())
    }

    async fn list_entities(&self, story: &str, kind: EntityKind) -> Reply<Vec<RemoteEntity>> {
        let listed: Vec<Value> = self
            .call(
                Method::GET,
                &format!("/v1/stories/{story}/{}", kind.path()),
                None,
                None,
            )
            .await?;
        listed.into_iter().map(Self::entity).collect()
    }

    async fn create_entity(
        &self,
        story: &str,
        kind: EntityKind,
        input: &Value,
    ) -> Reply<RemoteEntity> {
        Self::entity(
            self.call(
                Method::POST,
                &format!("/v1/stories/{story}/{}", kind.path()),
                None,
                Some(input),
            )
            .await?,
        )
    }

    async fn update_entity(
        &self,
        story: &str,
        kind: EntityKind,
        id: &str,
        revision: i64,
        input: &Value,
    ) -> Reply<RemoteEntity> {
        let path = format!("/v1/stories/{story}/{}/{id}", kind.path());
        Self::entity(
            self.call(Method::PATCH, &path, Some(revision), Some(input))
                .await?,
        )
    }

    async fn delete_entity(
        &self,
        story: &str,
        kind: EntityKind,
        id: &str,
        revision: i64,
    ) -> Reply<()> {
        let path = format!("/v1/stories/{story}/{}/{id}", kind.path());
        self.call(Method::DELETE, &path, Some(revision), None).await
    }

    async fn list_stories(&self) -> Reply<Vec<StoryListing>> {
        self.call(Method::GET, "/v1/stories", None, None).await
    }

    async fn read_chapters(&self, story: &str) -> Reply<Vec<ChapterText>> {
        self.call(
            Method::GET,
            &format!("/v1/stories/{story}/chapters"),
            None,
            None,
        )
        .await
    }

    fn hosts_image(&self, url: &str) -> bool {
        let ours = format!(
            "{}/v0/b/{}/o/",
            self.config.storage_url(),
            self.config.storage_bucket
        );
        url.starts_with(&ours)
    }

    async fn download_image(&self, url: &str) -> Reply<Vec<u8>> {
        if !self.hosts_image(url) {
            return Err(SyncError::NotFound);
        }
        let response = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|_| SyncError::Network)?;
        if !response.status().is_success() {
            return Err(SyncError::NotFound);
        }
        // Refuse an oversized body before reading it; `save_image` checks again.
        if response
            .content_length()
            .is_some_and(|size| size > crate::assets::MAX_IMAGE_BYTES as u64)
        {
            return Err(SyncError::Rejected("image too large".into()));
        }
        Ok(response
            .bytes()
            .await
            .map_err(|_| SyncError::Network)?
            .to_vec())
    }

    async fn upload_image(&self, story: &str, name: &str, bytes: Vec<u8>) -> Reply<String> {
        // The website counts uploads against a daily allowance before accepting one.
        let reserved = self
            .http
            .post(format!(
                "{}/reserveStorageUpload",
                self.config.functions_url()
            ))
            .bearer_auth(self.session.token.expose())
            .json(&json!({}))
            .send()
            .await
            .map_err(|_| SyncError::Network)?;
        if !reserved.status().is_success() {
            let status = reserved.status();
            let text = reserved.text().await.unwrap_or_default();
            return Err(match status.as_u16() {
                429 => SyncError::Rejected(message(&text)),
                _ => failure(status, &text),
            });
        }

        let path = format!("chapter-images/{}/{story}/{name}", self.session.account.uid);
        let object = format!(
            "{}/v0/b/{}/o",
            self.config.storage_url(),
            self.config.storage_bucket
        );
        let boundary = format!("tt-{}", crate::db::new_id());
        let uploaded = self
            .http
            .post(&object)
            .query(&[("name", &path)])
            .header(
                header::AUTHORIZATION,
                format!("Firebase {}", self.session.token.expose()),
            )
            .header("X-Goog-Upload-Protocol", "multipart")
            .header(
                header::CONTENT_TYPE,
                format!("multipart/related; boundary={boundary}"),
            )
            .body(multipart(&boundary, &path, mime(name), &bytes))
            .send()
            .await
            .map_err(|_| SyncError::Network)?;
        let status = uploaded.status();
        let text = uploaded.text().await.map_err(|_| SyncError::Network)?;
        if !status.is_success() {
            return Err(SyncError::Server(format!(
                "image upload failed (HTTP {})",
                status.as_u16()
            )));
        }
        let token = serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|json| json["downloadTokens"].as_str().map(str::to_string))
            .ok_or_else(|| SyncError::Server("image upload failed".into()))?;
        Ok(format!(
            "{object}/{}?alt=media&token={token}",
            encode(&path)
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failures_keep_story_datas_wording_and_separate_conflicts() {
        assert_eq!(
            failure(StatusCode::CONFLICT, r#"{"error":"revision conflict"}"#),
            SyncError::Conflict
        );
        assert_eq!(
            failure(StatusCode::UNAUTHORIZED, ""),
            SyncError::SignInNeeded
        );
        assert_eq!(
            failure(
                StatusCode::UNPROCESSABLE_ENTITY,
                r#"{"error":"a chapter can hold at most 5000 words"}"#
            ),
            SyncError::Rejected("a chapter can hold at most 5000 words".into())
        );
        assert_eq!(
            failure(StatusCode::BAD_GATEWAY, "<html>"),
            SyncError::Server("HTTP 502".into())
        );
    }

    #[test]
    fn uploads_are_described_the_way_firebase_storage_expects() {
        let body = multipart("B", "chapter-images/u/s/a.png", "image/png", &[1, 2, 3]);
        let text = String::from_utf8_lossy(&body);
        assert!(text.starts_with("--B\r\nContent-Type: application/json"));
        assert!(text.contains(r#""contentType":"image/png""#));
        assert!(text.ends_with("\r\n--B--"));
        assert_eq!(
            encode("chapter-images/u 1/a.png"),
            "chapter-images%2Fu%201%2Fa.png"
        );
        assert_eq!(mime("x.webp"), "image/webp");
        assert_eq!(mime("x.jpg"), "image/jpeg");
    }
}
