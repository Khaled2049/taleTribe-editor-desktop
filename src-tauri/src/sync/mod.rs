//! Signing in to the website and publishing a story to it. Like AI requests,
//! all of this leaves from the native side: the sign-in token never reaches the
//! webview, and no website address needs to allow the app's origin.

pub mod account;
pub mod config;
#[cfg(test)]
pub mod fake;
pub mod http;
pub mod import;
pub mod plan;
pub mod publish;
pub mod remote;

/// Failures while talking to the website. Messages are shown to the writer.
#[derive(Debug, PartialEq, thiserror::Error)]
pub enum SyncError {
    #[error("This build isn't set up to connect to TheTaleTribe.")]
    NotConfigured,
    #[error("Sign in to TheTaleTribe to continue.")]
    SignInNeeded,
    #[error("That email and password don't match an account.")]
    BadCredentials,
    #[error("Too many attempts. Wait a little and try again.")]
    TooManyAttempts,
    /// The record changed on the website since this app last saw it.
    #[error("This was changed on the website while publishing. Try again.")]
    Conflict,
    #[error("Not found on the website.")]
    NotFound,
    /// story-data's own wording, shown as written.
    #[error("{0}")]
    Rejected(String),
    #[error("The website is limiting requests right now. Try again in a few minutes.")]
    RateLimited,
    #[error("Couldn't reach TheTaleTribe. Check your connection.")]
    Network,
    #[error("The website returned an error: {0}")]
    Server(String),
}

impl SyncError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotConfigured => "notConfigured",
            Self::SignInNeeded => "signInNeeded",
            Self::BadCredentials => "badCredentials",
            Self::TooManyAttempts => "tooManyAttempts",
            Self::Conflict => "conflict",
            Self::NotFound => "notFound",
            Self::Rejected(_) => "rejected",
            Self::RateLimited => "rateLimited",
            Self::Network => "network",
            Self::Server(_) => "server",
        }
    }
}
