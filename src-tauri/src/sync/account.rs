//! The writer's website account. Sign-in goes through Firebase Authentication's
//! REST API with email and password, the one method the website offers. The
//! long-lived refresh token is kept in the keychain; the short-lived ID token
//! only ever lives in memory.

use std::time::{Duration, Instant};

use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::config::Config;
use super::SyncError;
use crate::error::AppResult;
use crate::secrets::Secret;

pub const REFRESH_TOKEN: &str = "website-refresh-token";
const ACCOUNT_KEY: &str = "account";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub uid: String,
    pub email: String,
}

/// A signed-in account with a token that is good for a little while.
#[derive(Debug, Clone)]
pub struct Session {
    pub account: Account,
    pub token: Secret,
    expires_at: Instant,
}

impl Session {
    /// Leaves a margin so a token cannot expire in the middle of a request.
    pub fn is_fresh(&self) -> bool {
        self.expires_at > Instant::now() + Duration::from_secs(60)
    }
}

#[derive(Debug)]
pub struct SignedIn {
    pub session: Session,
    pub refresh_token: Secret,
}

pub fn saved_account(conn: &Connection) -> AppResult<Option<Account>> {
    let stored: Option<String> = conn
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            [ACCOUNT_KEY],
            |row| row.get(0),
        )
        .optional()?;
    Ok(stored.and_then(|json| serde_json::from_str(&json).ok()))
}

pub fn save_account(conn: &Connection, account: &Account) -> AppResult<()> {
    let json = serde_json::to_string(account).expect("an account serializes");
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        [ACCOUNT_KEY, &json],
    )?;
    Ok(())
}

pub fn clear_account(conn: &Connection) -> AppResult<()> {
    conn.execute("DELETE FROM settings WHERE key = ?1", [ACCOUNT_KEY])?;
    Ok(())
}

fn signed_in(
    uid: &Value,
    email: &str,
    id_token: &Value,
    refresh: &Value,
    expires_in: &Value,
) -> Option<SignedIn> {
    let seconds: u64 = expires_in.as_str()?.parse().ok()?;
    Some(SignedIn {
        session: Session {
            account: Account {
                uid: uid.as_str()?.to_string(),
                email: email.to_string(),
            },
            token: Secret::new(id_token.as_str()?.to_string()),
            expires_at: Instant::now() + Duration::from_secs(seconds),
        },
        refresh_token: Secret::new(refresh.as_str()?.to_string()),
    })
}

/// Firebase reports failures as `{"error": {"message": "CODE"}}`.
fn auth_error(body: &Value, refreshing: bool) -> SyncError {
    let code = body["error"]["message"].as_str().unwrap_or("");
    let code = code.split([' ', ':']).next().unwrap_or("");
    match code {
        "TOO_MANY_ATTEMPTS_TRY_LATER" => SyncError::TooManyAttempts,
        _ if refreshing => SyncError::SignInNeeded,
        "INVALID_LOGIN_CREDENTIALS"
        | "EMAIL_NOT_FOUND"
        | "INVALID_PASSWORD"
        | "INVALID_EMAIL"
        | "MISSING_PASSWORD"
        | "USER_DISABLED" => SyncError::BadCredentials,
        _ => SyncError::Server("sign-in failed".into()),
    }
}

async fn post(request: reqwest::RequestBuilder) -> Result<(bool, Value), SyncError> {
    let response = request.send().await.map_err(|_| SyncError::Network)?;
    let ok = response.status().is_success();
    let body = response.json().await.unwrap_or(Value::Null);
    Ok((ok, body))
}

pub async fn sign_in(
    http: &reqwest::Client,
    config: &Config,
    email: &str,
    password: &str,
) -> Result<SignedIn, SyncError> {
    if !config.is_configured() {
        return Err(SyncError::NotConfigured);
    }
    let url = format!("{}/v1/accounts:signInWithPassword", config.auth_url());
    let request = http
        .post(url)
        .query(&[("key", config.api_key())])
        .json(&json!({ "email": email, "password": password, "returnSecureToken": true }));
    let (ok, body) = post(request).await?;
    if !ok {
        return Err(auth_error(&body, false));
    }
    signed_in(
        &body["localId"],
        email,
        &body["idToken"],
        &body["refreshToken"],
        &body["expiresIn"],
    )
    .ok_or_else(|| SyncError::Server("sign-in failed".into()))
}

/// Trades the stored refresh token for a new ID token. A refusal means the
/// account was signed out elsewhere, disabled or deleted.
pub async fn refresh(
    http: &reqwest::Client,
    config: &Config,
    account: &Account,
    refresh_token: &Secret,
) -> Result<SignedIn, SyncError> {
    let url = format!("{}/v1/token", config.token_url());
    let request = http.post(url).query(&[("key", config.api_key())]).form(&[
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token.expose()),
    ]);
    let (ok, body) = post(request).await?;
    if !ok {
        return Err(auth_error(&body, true));
    }
    let renewed = signed_in(
        &body["user_id"],
        &account.email,
        &body["id_token"],
        &body["refresh_token"],
        &body["expires_in"],
    )
    .ok_or(SyncError::SignInNeeded)?;
    if renewed.session.account.uid != account.uid {
        return Err(SyncError::SignInNeeded);
    }
    Ok(renewed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;

    #[test]
    fn sign_in_answers_become_a_session_without_exposing_tokens() {
        let body = json!({"localId": "u1", "idToken": "secret-id", "refreshToken": "secret-refresh", "expiresIn": "3600"});
        let signed = signed_in(
            &body["localId"],
            "a@b.c",
            &body["idToken"],
            &body["refreshToken"],
            &body["expiresIn"],
        )
        .unwrap();
        assert_eq!(
            signed.session.account,
            Account {
                uid: "u1".into(),
                email: "a@b.c".into()
            }
        );
        assert!(signed.session.is_fresh());
        assert!(!format!("{signed:?}").contains("secret-"));

        let short = json!("30");
        let brief = signed_in(
            &body["localId"],
            "a@b.c",
            &body["idToken"],
            &body["refreshToken"],
            &short,
        )
        .unwrap();
        assert!(
            !brief.session.is_fresh(),
            "a token about to expire is renewed first"
        );
    }

    #[test]
    fn firebase_failures_map_to_what_the_writer_should_do() {
        let failure = |code: &str| json!({"error": {"message": code}});
        assert_eq!(
            auth_error(&failure("INVALID_LOGIN_CREDENTIALS"), false),
            SyncError::BadCredentials
        );
        assert_eq!(
            auth_error(
                &failure("TOO_MANY_ATTEMPTS_TRY_LATER : Access disabled"),
                false
            ),
            SyncError::TooManyAttempts
        );
        assert_eq!(
            auth_error(&failure("TOKEN_EXPIRED"), true),
            SyncError::SignInNeeded
        );
        assert_eq!(
            auth_error(&failure("USER_NOT_FOUND"), true),
            SyncError::SignInNeeded
        );
    }

    #[test]
    fn the_saved_account_holds_no_token() {
        let conn = open_in_memory().unwrap();
        assert_eq!(saved_account(&conn).unwrap(), None);
        let account = Account {
            uid: "u1".into(),
            email: "a@b.c".into(),
        };
        save_account(&conn, &account).unwrap();
        assert_eq!(saved_account(&conn).unwrap(), Some(account));
        let stored: String = conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'account'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(stored, r#"{"uid":"u1","email":"a@b.c"}"#);
        clear_account(&conn).unwrap();
        assert_eq!(saved_account(&conn).unwrap(), None);
    }
}
