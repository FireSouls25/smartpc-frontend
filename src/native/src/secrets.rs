//! Secrets live in the OS credential store — Keychain on macOS,
//! Credential Manager on Windows, Secret Service on Linux.
//! Never in SQLite, never in logs, never in API responses.
//! `<PROVIDER>_API_KEY` env vars (e.g. OPENCODE_API_KEY) override for
//! containers and CI where no keyring exists. Env wins when present.
pub fn account(user_id: &str, provider: &str) -> String {
    format!("{user_id}:{provider}-api-key")
}

fn env_key(provider: &str) -> Option<String> {
    let var = format!(
        "{}_API_KEY",
        provider.to_uppercase().replace('.', "_").replace('-', "_")
    );
    std::env::var(var).ok().filter(|v| !v.trim().is_empty())
}

pub fn get_key(user_id: &str, provider: &str) -> Option<String> {
    if let Some(k) = env_key(provider) {
        return Some(k);
    }
    keyring::Entry::new("smart-pc", &account(user_id, provider))
        .ok()?
        .get_password()
        .ok()
        .filter(|v| !v.trim().is_empty())
}

pub fn has_key(user_id: &str, provider: &str) -> bool {
    get_key(user_id, provider).is_some()
}

pub fn set_key(user_id: &str, provider: &str, key: &str) -> Result<(), String> {
    let key = key.trim();
    if key.len() < 8 {
        return Err("key too short".into());
    }
    keyring::Entry::new("smart-pc", &account(user_id, provider))
        .map_err(|e| format!("credential store unavailable: {e}"))?
        .set_password(key)
        .map_err(|e| format!("could not save key: {e}"))
}

pub fn delete_key(user_id: &str, provider: &str) -> Result<(), String> {
    let entry = keyring::Entry::new("smart-pc", &account(user_id, provider))
        .map_err(|e| format!("credential store unavailable: {e}"))?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("could not delete key: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_short_keys_without_touching_the_store() {
        assert!(set_key("u", "opencode", "  short ").is_err());
        assert!(set_key("u", "opencode", "").is_err());
    }

    #[test]
    fn account_names_are_user_scoped() {
        assert_ne!(account("alice", "opencode"), account("bob", "opencode"));
    }
}
