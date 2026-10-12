//! Secrets: API keys and the website sign-in token. They are written to the
//! operating system's keychain and read back only on the native side, at the
//! moment a request is sent. Nothing here is stored in SQLite.

use std::collections::HashMap;
use std::fmt;
use std::sync::{Mutex, MutexGuard};

const SERVICE: &str = "com.thetaletribe.desktop";

/// A secret that cannot be printed by accident through `{:?}` or a log line.
#[derive(Clone)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: String) -> Self {
        Self(value)
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(redacted)")
    }
}

/// The keychain could not be used. The text names the item, never its contents.
#[derive(Debug, PartialEq, thiserror::Error)]
#[error("Couldn't use the system keychain: {0}")]
pub struct KeychainError(pub String);

pub trait SecretStore: Send + Sync {
    fn get(&self, name: &str) -> Result<Option<Secret>, KeychainError>;
    fn set(&self, name: &str, value: &str) -> Result<(), KeychainError>;
    fn delete(&self, name: &str) -> Result<(), KeychainError>;
}

pub struct Keychain;

impl Keychain {
    fn entry(name: &str) -> Result<keyring::Entry, KeychainError> {
        keyring::Entry::new(SERVICE, name).map_err(failed)
    }
}

fn failed(error: keyring::Error) -> KeychainError {
    KeychainError(error.to_string())
}

impl SecretStore for Keychain {
    fn get(&self, name: &str) -> Result<Option<Secret>, KeychainError> {
        match Self::entry(name)?.get_password() {
            Ok(value) => Ok(Some(Secret::new(value))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(failed(error)),
        }
    }

    fn set(&self, name: &str, value: &str) -> Result<(), KeychainError> {
        Self::entry(name)?.set_password(value).map_err(failed)
    }

    fn delete(&self, name: &str) -> Result<(), KeychainError> {
        match Self::entry(name)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(failed(error)),
        }
    }
}

/// Remembers what the keychain said for as long as the app runs.
///
/// Every keychain access can make the operating system ask the writer for
/// permission, so each secret is read at most once per launch, and a write or
/// delete that would change nothing is not sent at all.
pub struct Cached<S> {
    inner: S,
    seen: Mutex<HashMap<String, Option<String>>>,
}

impl<S> Cached<S> {
    pub fn new(inner: S) -> Self {
        Self {
            inner,
            seen: Mutex::new(HashMap::new()),
        }
    }

    fn seen(&self) -> MutexGuard<'_, HashMap<String, Option<String>>> {
        self.seen
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl<S: SecretStore> SecretStore for Cached<S> {
    fn get(&self, name: &str) -> Result<Option<Secret>, KeychainError> {
        if let Some(known) = self.seen().get(name) {
            return Ok(known.clone().map(Secret::new));
        }
        let found = self.inner.get(name)?;
        self.seen()
            .insert(name.into(), found.as_ref().map(|s| s.expose().to_string()));
        Ok(found)
    }

    fn set(&self, name: &str, value: &str) -> Result<(), KeychainError> {
        if self.seen().get(name) == Some(&Some(value.to_string())) {
            return Ok(());
        }
        self.inner.set(name, value)?;
        self.seen().insert(name.into(), Some(value.into()));
        Ok(())
    }

    fn delete(&self, name: &str) -> Result<(), KeychainError> {
        if self.seen().get(name) == Some(&None) {
            return Ok(());
        }
        self.inner.delete(name)?;
        self.seen().insert(name.into(), None);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A keychain that counts how often it is touched.
    #[derive(Default)]
    struct Counting {
        items: Mutex<HashMap<String, String>>,
        calls: Mutex<u32>,
    }

    impl Counting {
        fn touch(&self) -> MutexGuard<'_, HashMap<String, String>> {
            *self.calls.lock().unwrap() += 1;
            self.items.lock().unwrap()
        }
    }

    impl SecretStore for &Counting {
        fn get(&self, name: &str) -> Result<Option<Secret>, KeychainError> {
            Ok(self.touch().get(name).cloned().map(Secret::new))
        }

        fn set(&self, name: &str, value: &str) -> Result<(), KeychainError> {
            self.touch().insert(name.into(), value.into());
            Ok(())
        }

        fn delete(&self, name: &str) -> Result<(), KeychainError> {
            self.touch().remove(name);
            Ok(())
        }
    }

    #[test]
    fn the_keychain_is_asked_once_per_secret_and_never_for_a_no_op() {
        let keychain = Counting::default();
        keychain
            .items
            .lock()
            .unwrap()
            .insert("token".into(), "abc".into());
        let secrets = Cached::new(&keychain);
        let calls = || *keychain.calls.lock().unwrap();

        for _ in 0..3 {
            assert_eq!(secrets.get("token").unwrap().unwrap().expose(), "abc");
            assert!(secrets.get("missing").unwrap().is_none());
        }
        assert_eq!(
            calls(),
            2,
            "one read each, however often they are asked for"
        );

        secrets.set("token", "abc").unwrap();
        secrets.delete("missing").unwrap();
        assert_eq!(
            calls(),
            2,
            "writing the same value or deleting nothing is skipped"
        );

        secrets.set("token", "new").unwrap();
        assert_eq!(calls(), 3);
        assert_eq!(secrets.get("token").unwrap().unwrap().expose(), "new");
        assert_eq!(keychain.items.lock().unwrap()["token"], "new");
        secrets.delete("token").unwrap();
        assert!(secrets.get("token").unwrap().is_none());
        assert_eq!(calls(), 4);
    }

    #[test]
    fn a_secret_never_appears_in_debug_output() {
        let secret = Secret::new("sk-ant-very-private".into());
        let printed = format!("{secret:?} {:?}", Some(&secret));
        assert!(!printed.contains("very-private"));
        assert_eq!(secret.expose(), "sk-ant-very-private");
    }
}
