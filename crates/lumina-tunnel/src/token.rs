//! The ngrok authtoken, kept in the OS credential store (Windows Credential
//! Manager, macOS Keychain, Secret Service), never in config.toml.

use crate::{Error, Result};

/// One secret in the OS credential store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenStore {
    service: String,
    account: String,
}

impl TokenStore {
    /// Where Lumina keeps the ngrok authtoken.
    pub fn ngrok() -> Self {
        Self::new("dev.lumina.app", "ngrok-authtoken")
    }

    pub fn new(service: &str, account: &str) -> Self {
        Self {
            service: service.to_owned(),
            account: account.to_owned(),
        }
    }

    fn entry(&self) -> Result<keyring::Entry> {
        keyring::Entry::new(&self.service, &self.account).map_err(keychain)
    }

    /// The saved token, if any.
    pub async fn load(&self) -> Result<Option<String>> {
        let store = self.clone();
        blocking(move || match store.entry()?.get_password() {
            Ok(token) => Ok(Some(token)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(keychain(e)),
        })
        .await
    }

    /// Checks and saves `token`, replacing a saved one.
    pub async fn save(&self, token: &str) -> Result<()> {
        let token = validate(token)?;
        let store = self.clone();
        blocking(move || store.entry()?.set_password(&token).map_err(keychain)).await
    }

    /// Removes the saved token (nothing happens if there is none).
    pub async fn delete(&self) -> Result<()> {
        let store = self.clone();
        blocking(move || match store.entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(keychain(e)),
        })
        .await
    }
}

fn keychain(e: keyring::Error) -> Error {
    Error::Keychain(e.to_string())
}

/// Credential stores block (and the Secret Service runs its own executor):
/// keep them off the async runtime's threads.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| Error::Keychain(e.to_string()))?
}

/// A pasted authtoken without surrounding spaces, or why it cannot be one.
/// ngrok tokens are base62 with `_` (the dashboard's "Your Authtoken");
/// people also paste the whole `ngrok config add-authtoken <token>` command.
pub fn validate(input: &str) -> Result<String> {
    let token = input
        .trim()
        .rsplit(char::is_whitespace)
        .next()
        .unwrap_or_default();
    if token.is_empty() {
        return Err(Error::InvalidToken("the authtoken is empty".into()));
    }
    if token.len() < 20 || token.len() > 200 {
        return Err(Error::InvalidToken(
            "this does not look like an ngrok authtoken (wrong length)".into(),
        ));
    }
    if !token
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(Error::InvalidToken(
            "an ngrok authtoken contains only letters, digits, `_` and `-`".into(),
        ));
    }
    Ok(token.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &str = "2abcDEFghiJKLmnoPQRstu_3vwxYZ0123456789abcdEFGH";

    #[test]
    fn accepts_a_token_or_the_whole_command() {
        assert_eq!(validate(&format!("  {TOKEN}\n")).expect("token"), TOKEN);
        assert_eq!(
            validate(&format!("ngrok config add-authtoken {TOKEN}")).expect("command"),
            TOKEN
        );
    }

    #[test]
    fn refuses_what_cannot_be_a_token() {
        for bad in [
            "",
            "   ",
            "short",
            "has;semicolon_and_is_long_enough_123",
            &"x".repeat(201),
        ] {
            assert!(
                matches!(validate(bad), Err(Error::InvalidToken(_))),
                "{bad:?}"
            );
        }
    }

    /// Round trip through the real credential store; opt-in, since CI
    /// machines may have no keychain (`LUMINA_TEST_KEYCHAIN=1`).
    #[tokio::test]
    async fn keychain_round_trip() {
        if std::env::var_os("LUMINA_TEST_KEYCHAIN").is_none() {
            eprintln!("skipped: set LUMINA_TEST_KEYCHAIN=1");
            return;
        }
        let store = TokenStore::new("dev.lumina.app.test", "tunnel-test");
        store.delete().await.expect("clean");
        assert_eq!(store.load().await.expect("load"), None);
        store.save(TOKEN).await.expect("save");
        assert_eq!(store.load().await.expect("load").as_deref(), Some(TOKEN));
        store.delete().await.expect("delete");
        store.delete().await.expect("delete twice");
        assert_eq!(store.load().await.expect("load"), None);
    }
}
