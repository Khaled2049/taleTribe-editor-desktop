//! Which model the writer uses. Stored as one JSON row; it has no field that
//! could hold an API key.

use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::Provider;
use crate::error::{AppError, AppResult};

const KEY: &str = "ai";
pub const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ModelChoice {
    pub id: String,
    /// Anthropic: the model accepts an effort level.
    pub supports_effort: bool,
    /// Ollama: the model reasons before answering unless told not to.
    pub can_think: bool,
}

/// How much story background goes into a prompt. Local models with small
/// context windows need the small setting.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContextBudget {
    Small,
    #[default]
    Medium,
    Large,
}

impl ContextBudget {
    /// Characters, at roughly four per token: about 2k, 6k and 16k tokens.
    pub fn chars(self) -> usize {
        match self {
            Self::Small => 8_000,
            Self::Medium => 24_000,
            Self::Large => 64_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AiSettings {
    /// `None` until the writer sets AI up.
    pub provider: Option<Provider>,
    pub ollama_url: String,
    pub ollama: ModelChoice,
    pub anthropic: ModelChoice,
    pub openai: ModelChoice,
    pub context_budget: ContextBudget,
}

impl Default for AiSettings {
    fn default() -> Self {
        Self {
            provider: None,
            ollama_url: DEFAULT_OLLAMA_URL.into(),
            ollama: ModelChoice::default(),
            anthropic: ModelChoice::default(),
            openai: ModelChoice::default(),
            context_budget: ContextBudget::default(),
        }
    }
}

impl AiSettings {
    pub fn model(&self, provider: Provider) -> &ModelChoice {
        match provider {
            Provider::Ollama => &self.ollama,
            Provider::Anthropic => &self.anthropic,
            Provider::Openai => &self.openai,
        }
    }
}

/// Trims the address and checks it is a plain http(s) URL.
pub fn normalize_ollama_url(url: &str) -> AppResult<String> {
    let url = url.trim().trim_end_matches('/');
    let valid = reqwest::Url::parse(url)
        .is_ok_and(|parsed| matches!(parsed.scheme(), "http" | "https") && parsed.has_host());
    if !valid {
        return Err(AppError::validation(
            "The Ollama address must look like http://localhost:11434",
        ));
    }
    Ok(url.to_string())
}

pub fn load(conn: &Connection) -> AppResult<AiSettings> {
    let stored: Option<String> = conn
        .query_row("SELECT value FROM settings WHERE key = ?1", [KEY], |row| {
            row.get(0)
        })
        .optional()?;
    // Unreadable settings fall back to defaults rather than locking the writer out.
    Ok(stored
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default())
}

pub fn save(conn: &Connection, mut settings: AiSettings) -> AppResult<AiSettings> {
    settings.ollama_url = normalize_ollama_url(&settings.ollama_url)?;
    let json = serde_json::to_string(&settings).map_err(|e| AppError::validation(e.to_string()))?;
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        [KEY, &json],
    )?;
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;

    #[test]
    fn settings_round_trip_and_reject_a_bad_ollama_address() {
        let conn = open_in_memory().unwrap();
        assert_eq!(load(&conn).unwrap(), AiSettings::default());

        let mut settings = AiSettings {
            provider: Some(Provider::Ollama),
            ollama_url: " http://localhost:11434/ ".into(),
            ..AiSettings::default()
        };
        settings.ollama.id = "qwen3.5:9b".into();
        let saved = save(&conn, settings).unwrap();
        assert_eq!(saved.ollama_url, "http://localhost:11434");
        assert_eq!(load(&conn).unwrap(), saved);

        let bad = AiSettings {
            ollama_url: "file:///etc/passwd".into(),
            ..AiSettings::default()
        };
        assert!(save(&conn, bad).is_err());
    }
}
