use cosmic::cosmic_config::ConfigSet;

use super::{App, entry::TotpEntry};

pub enum PassphraseState {
    Inputting { input: String, hidden: bool },
    Recieved(age::secrecy::SecretString),
}

pub enum SecretState {
    NoSecretsFile,
    RequestingPassphrase { secret_data: Vec<u8> },
    LoadedSecrets { entries: Vec<TotpEntry> },
}

impl App {
    pub fn try_save_secrets(&mut self) -> Result<(), String> {
        if let SecretState::LoadedSecrets { entries } = &self.secret_state {
            let s = serde_json::to_string(entries)
                .map_err(|e| format!("Could not serialize secrets: {e}"))?;

            let PassphraseState::Recieved(pass) = &self.passphrase else {
                return Err("Password has not been set, unable to encrypt secrets".into());
            };

            let s = match age::encrypt(&age::scrypt::Recipient::new(pass.clone()), s.as_bytes()) {
                Ok(s) => s,
                Err(e) => return Err(format!("Could not encrypt secrets: {e}")),
            };

            if let Err(e) = self.config.set("secrets", s) {
                return Err(format!("Could not save secrest file: {e}"));
            };
        }

        Ok(())
    }

    pub fn try_decode_secrets(&mut self) -> Result<(), String> {
        match (&self.secret_state, &self.passphrase) {
            (SecretState::NoSecretsFile, PassphraseState::Recieved(s)) => {
                _ = s;
                self.secret_state = SecretState::LoadedSecrets {
                    entries: Vec::new(),
                };
                Ok(())
            }
            (SecretState::RequestingPassphrase { secret_data }, PassphraseState::Recieved(s)) => {
                let secr = match age::decrypt(&age::scrypt::Identity::new(s.clone()), secret_data) {
                    Ok(s) => s,
                    Err(e) => {
                        return Err(format!(
                            "Could not decrypt secrets file - likey invalid passphrase: {e}"
                        ));
                    }
                };

                let secr = match serde_json::from_slice(&secr) {
                    Ok(v) => v,
                    Err(e) => {
                        return Err(format!("Could not deserialize from secrets file: {e}"));
                    }
                };

                self.secret_state = SecretState::LoadedSecrets { entries: secr };

                Ok(())
            }
            _ => Ok(()),
        }
    }
}
