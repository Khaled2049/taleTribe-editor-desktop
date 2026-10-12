//! Where the website's services are. Values are fixed at build time through
//! `TT_*` environment variables; with none set, a debug build talks to the
//! local stack started by `./dev-new.sh`.

pub struct Config {
    pub story_data_url: String,
    pub web_url: String,
    pub firebase_api_key: String,
    pub project_id: String,
    pub storage_bucket: String,
    /// Use the Firebase emulators, and story-data's `AUTH_MODE=dev` header.
    pub emulators: bool,
}

fn var(value: Option<&'static str>, fallback: &str) -> String {
    value.unwrap_or(fallback).trim_end_matches('/').to_string()
}

impl Config {
    pub fn from_build() -> Self {
        let project_id = var(option_env!("TT_FIREBASE_PROJECT_ID"), "story-6f89f");
        Self {
            story_data_url: var(option_env!("TT_STORY_DATA_URL"), "http://localhost:8084"),
            web_url: var(option_env!("TT_WEB_URL"), "http://localhost:5173"),
            firebase_api_key: var(option_env!("TT_FIREBASE_API_KEY"), ""),
            storage_bucket: var(
                option_env!("TT_STORAGE_BUCKET"),
                &format!("{project_id}.appspot.com"),
            ),
            project_id,
            emulators: match option_env!("TT_USE_EMULATORS") {
                Some(value) => value == "true",
                None => cfg!(debug_assertions),
            },
        }
    }

    /// A release build made without its website settings cannot sign in.
    pub fn is_configured(&self) -> bool {
        self.emulators || !self.firebase_api_key.is_empty()
    }

    pub fn api_key(&self) -> &str {
        // The emulator accepts any key but requires one.
        if self.firebase_api_key.is_empty() {
            "local-emulator"
        } else {
            &self.firebase_api_key
        }
    }

    pub fn auth_url(&self) -> &'static str {
        if self.emulators {
            "http://127.0.0.1:9099/identitytoolkit.googleapis.com"
        } else {
            "https://identitytoolkit.googleapis.com"
        }
    }

    pub fn token_url(&self) -> &'static str {
        if self.emulators {
            "http://127.0.0.1:9099/securetoken.googleapis.com"
        } else {
            "https://securetoken.googleapis.com"
        }
    }

    pub fn storage_url(&self) -> &'static str {
        if self.emulators {
            "http://127.0.0.1:9199"
        } else {
            "https://firebasestorage.googleapis.com"
        }
    }

    pub fn functions_url(&self) -> String {
        if self.emulators {
            format!("http://127.0.0.1:5001/{}/us-central1", self.project_id)
        } else {
            format!("https://us-central1-{}.cloudfunctions.net", self.project_id)
        }
    }
}
