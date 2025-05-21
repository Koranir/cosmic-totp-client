use tracing::{info, warn};

use super::entry::Entry;

#[derive(Debug, Clone)]
pub enum State {
    PendingUser,
    Secrets(Vec<Entry>),
}
impl State {
    pub fn as_mut_array(&mut self) -> &mut [Entry] {
        match self {
            Self::PendingUser => &mut [],
            Self::Secrets(items) => &mut *items,
        }
    }
    pub fn as_array(&self) -> &[Entry] {
        match self {
            Self::PendingUser => &[],
            Self::Secrets(items) => items,
        }
    }

    #[expect(clippy::result_large_err)]
    pub fn try_push(&mut self, entry: Entry) -> Result<(), Entry> {
        match self {
            Self::PendingUser => Err(entry),
            Self::Secrets(items) => {
                items.push(entry);
                Ok(())
            }
        }
    }
}

pub async fn get_secret_key(username: String) -> Result<State, String> {
    let data = tokio::task::spawn_blocking(move || {
        info!("Requesting secrets");
        let entry = keyring::Entry::new(crate::APP_ID, &username).map_err(|e| e.to_string())?;
        Ok(match entry.get_secret() {
            Ok(secr) => serde_json::from_slice(&secr)
                .map_err(|e| format!("Couldn't deserialise secret store: {e}"))?,
            Err(keyring::Error::NoEntry) => {
                warn!("No entry in secret store, defaulting to empty");
                Vec::new()
            }
            Err(e) => return Err(e.to_string()),
        })
    })
    .await
    .map_err(|e| format!("Couldn't join secret retrieving thread: {e}"))??;

    info!("Retrieved secret key");
    Ok(State::Secrets(data))
}

pub async fn set_secret_key(username: String, secret: Vec<Entry>) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        info!("Setting secrets");
        let entry = keyring::Entry::new(crate::APP_ID, &username).map_err(|e| e.to_string())?;
        let ser = dbg!(serde_json::to_string(&secret))
            .map_err(|e| format!("Failed to serialise secrets: {e}"))?;

        match entry.set_secret(ser.as_bytes()) {
            Ok(attrs) => attrs,
            Err(e) => return Err(e.to_string()),
        }
        Ok(())
    })
    .await
    .map_err(|e| format!("Couldn't join secret retrieving thread: {e}"))??;

    info!("Set secret key");
    Ok(())
}
