//! Writing help from a model the writer chose: a local one through Ollama, or
//! Anthropic or OpenAI with their own key. Requests leave from here, the native
//! side, so keys and story-wide context never pass through the webview.

pub mod context;
pub mod prompts;
pub mod providers;
pub mod settings;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Ollama,
    Anthropic,
    Openai,
}

impl Provider {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ollama => "ollama",
            Self::Anthropic => "anthropic",
            Self::Openai => "openai",
        }
    }

    /// Name of this provider's API key in the keychain.
    pub fn key_name(self) -> String {
        format!("{}-api-key", self.as_str())
    }

    pub fn needs_key(self) -> bool {
        !matches!(self, Self::Ollama)
    }
}

/// Failures the writer can act on. Messages are shown as written and never
/// include story text, prompts or keys.
#[derive(Debug, PartialEq, thiserror::Error)]
pub enum AiError {
    #[error("AI is not set up yet. Choose a model in Settings.")]
    NotConfigured,
    #[error("No API key is saved for this provider. Add one in Settings.")]
    MissingKey,
    #[error("Couldn't reach Ollama at {0}. Check that Ollama is running.")]
    OllamaUnreachable(String),
    #[error("The model \"{0}\" isn't available. Pick another in Settings.")]
    ModelMissing(String),
    #[error("The provider rejected the API key. Check it in Settings.")]
    BadKey,
    #[error("The provider is limiting requests right now. Try again in a moment.")]
    RateLimited,
    #[error("The model declined this request.")]
    Refused,
    #[error("Couldn't reach the provider. Check your connection.")]
    Network,
    #[error("The provider returned an error: {0}")]
    Provider(String),
    #[error("The model's answer couldn't be read. Try again.")]
    BadAnswer,
    #[error("Cancelled.")]
    Cancelled,
}

impl AiError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotConfigured => "notConfigured",
            Self::MissingKey => "missingKey",
            Self::OllamaUnreachable(_) => "ollamaUnreachable",
            Self::ModelMissing(_) => "modelMissing",
            Self::BadKey => "badKey",
            Self::RateLimited => "rateLimited",
            Self::Refused => "refused",
            Self::Network => "network",
            Self::Provider(_) => "provider",
            Self::BadAnswer => "badAnswer",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EnhanceAction {
    Expand,
    Dialogue,
    Rewrite,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IdeaKind {
    Characters,
    Plots,
    Places,
    Themes,
}

/// What the webview asks for. It sends only the text under the cursor or
/// selection, which may be newer than the last save; everything else about the
/// story is read from the database here.
#[derive(Debug, Deserialize)]
#[serde(
    tag = "task",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Task {
    NextLines {
        story_id: String,
        chapter_id: String,
        before: String,
        after: String,
    },
    Enhance {
        story_id: String,
        action: EnhanceAction,
        text: String,
    },
    Brainstorm {
        story_id: String,
        kind: IdeaKind,
        #[serde(default)]
        prompt: String,
    },
}
