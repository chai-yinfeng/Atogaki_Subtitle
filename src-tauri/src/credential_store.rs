use std::fmt::Debug;

use anyhow::{Context, Result};

const SERVICE_NAME: &str = "com.chai-yinfeng.atogaki.translation";

#[derive(Debug, Clone, Default)]
pub struct SystemCredentialStore;

pub trait CredentialStore: Debug + Send + Sync {
    fn backend_name(&self) -> &'static str;
    fn get(&self, provider_id: &str) -> Result<Option<String>>;
    fn set(&self, provider_id: &str, secret: &str) -> Result<()>;
    fn delete(&self, provider_id: &str) -> Result<()>;
}

impl CredentialStore for SystemCredentialStore {
    fn backend_name(&self) -> &'static str {
        #[cfg(target_os = "macos")]
        {
            "macOS Keychain"
        }
        #[cfg(target_os = "windows")]
        {
            "Windows Credential Manager"
        }
        #[cfg(target_os = "linux")]
        {
            "Secret Service"
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            "system credential store"
        }
    }

    fn get(&self, provider_id: &str) -> Result<Option<String>> {
        let entry = keyring::Entry::new(SERVICE_NAME, provider_id)
            .context("failed to open the system credential entry")?;
        match entry.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(error).context("failed to read from the system credential store"),
        }
    }

    fn set(&self, provider_id: &str, secret: &str) -> Result<()> {
        let entry = keyring::Entry::new(SERVICE_NAME, provider_id)
            .context("failed to open the system credential entry")?;
        entry
            .set_password(secret)
            .context("failed to save to the system credential store")
    }

    fn delete(&self, provider_id: &str) -> Result<()> {
        let entry = keyring::Entry::new(SERVICE_NAME, provider_id)
            .context("failed to open the system credential entry")?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(error).context("failed to delete from the system credential store"),
        }
    }
}
