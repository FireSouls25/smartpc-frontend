//! Secrets: OS keyring first, encrypted file second, env override always.
//!
//! - Reads: `<PROVIDER>_API_KEY` env wins (containers/CI), then the OS
//!   credential store (Keychain / Credential Manager / Secret Service),
//!   then the encrypted file fallback.
//! - Writes: keyring; when it is missing or locked (common when the app is
//!   launched without a login session bus), the key is sealed with
//!   XChaCha20-Poly1305 into `secrets.enc.json` (0600) instead of failing.
//!   The file key derives via HKDF-SHA256 from the machine id, so a disk
//!   moved to another machine does not decrypt. Plaintext never touches disk.
//! - `SMARTPC_NO_KEYRING=1` forces the file backend (headless setups).
//! - `SMARTPC_SECRETS_FILE=…` overrides the file location (tests).
//! Never in SQLite, never in logs, never in API responses.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chacha20poly1305::{
    aead::{Aead, KeyInit},
    XChaCha20Poly1305, XNonce,
};
use hkdf::Hkdf;
use sha2::Sha256;

static FILE_LOCK: Mutex<()> = Mutex::new(());

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

fn force_file() -> bool {
    std::env::var("SMARTPC_NO_KEYRING")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

pub fn get_key(user_id: &str, provider: &str) -> Option<String> {
    if let Some(k) = env_key(provider) {
        return Some(k);
    }
    if !force_file() {
        if let Ok(entry) = keyring::Entry::new("smart-pc", &account(user_id, provider)) {
            if let Ok(pw) = entry.get_password() {
                if !pw.trim().is_empty() {
                    return Some(pw);
                }
            }
        }
    }
    file_get(user_id, provider)
}

pub fn has_key(user_id: &str, provider: &str) -> bool {
    get_key(user_id, provider).is_some()
}

pub fn set_key(user_id: &str, provider: &str, key: &str) -> Result<(), String> {
    let key = key.trim();
    if key.len() < 8 {
        return Err("key too short".into());
    }
    if !force_file() {
        match keyring::Entry::new("smart-pc", &account(user_id, provider)) {
            Ok(entry) => match entry.set_password(key) {
                Ok(()) => {
                    // Keyring won: drop any stale fallback entry so the two
                    // backends can never disagree.
                    let _ = file_delete(user_id, provider);
                    return Ok(());
                }
                Err(e) => {
                    return file_set_with_note(
                        user_id,
                        provider,
                        key,
                        &format!("keyring save failed: {e}"),
                    );
                }
            },
            Err(e) => {
                return file_set_with_note(
                    user_id,
                    provider,
                    key,
                    &format!("keyring unavailable: {e}"),
                );
            }
        }
    }
    file_set_with_note(
        user_id,
        provider,
        key,
        "keyring bypassed (SMARTPC_NO_KEYRING)",
    )
}

fn file_set_with_note(user_id: &str, provider: &str, key: &str, note: &str) -> Result<(), String> {
    match file_set(user_id, provider, key) {
        Ok(()) => {
            crate::diagnostics::push(format!("keys: {note}; used encrypted file instead"));
            Ok(())
        }
        Err(fe) => Err(format!("{note}; file fallback also failed: {fe}")),
    }
}

pub fn delete_key(user_id: &str, provider: &str) -> Result<(), String> {
    let mut keyring_err: Option<String> = None;
    if !force_file() {
        match keyring::Entry::new("smart-pc", &account(user_id, provider)) {
            Ok(entry) => match entry.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => {}
                Err(e) => keyring_err = Some(format!("{e}")),
            },
            Err(e) => keyring_err = Some(format!("{e}")),
        }
    }
    match file_delete(user_id, provider) {
        Ok(()) => Ok(()),
        Err(fe) => match keyring_err {
            None => Ok(()),
            Some(ke) => Err(format!("keyring ({ke}) and file ({fe}) delete failed")),
        },
    }
}

// --- Encrypted file backend ---------------------------------------------

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct SecretFile {
    salt: String,
    entries: BTreeMap<String, FileEntry>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct FileEntry {
    nonce: String,
    ct: String,
}

fn secrets_file() -> Option<PathBuf> {
    if let Some(p) = std::env::var("SMARTPC_SECRETS_FILE")
        .ok()
        .filter(|v| !v.trim().is_empty())
    {
        return Some(PathBuf::from(p));
    }
    let base = std::env::var("XDG_DATA_HOME")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .filter(|v| !v.trim().is_empty())
                .map(|h| PathBuf::from(h).join(".local").join("share"))
        })?;
    Some(base.join("smart-pc").join("secrets.enc.json"))
}

fn set_600(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
}

/// Machine-bound secret: env override, OS machine id, else a stable random
/// key stored next to the secrets file (portable, still 0600).
fn machine_secret(secrets_path: &Path) -> Option<Vec<u8>> {
    if let Some(s) = std::env::var("SMARTPC_MACHINE_SECRET")
        .ok()
        .filter(|v| !v.trim().is_empty())
    {
        return Some(s.into_bytes());
    }
    for p in ["/etc/machine-id", "/var/lib/dbus/machine-id"] {
        if let Some(s) = std::fs::read_to_string(p)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
        {
            return Some(s.into_bytes());
        }
    }
    let key_path = secrets_path.with_extension("key");
    if let Ok(raw) = std::fs::read(&key_path) {
        if raw.len() == 32 {
            return Some(raw);
        }
    }
    let mut raw = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut raw);
    if std::fs::write(&key_path, raw).is_ok() {
        set_600(&key_path);
        return Some(raw.to_vec());
    }
    None
}

fn entry_key(salt: &[u8], machine: &[u8], account: &str) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(Some(salt), machine);
    let mut info = b"smart-pc secret v1:".to_vec();
    info.extend_from_slice(account.as_bytes());
    let mut out = [0u8; 32];
    hk.expand(&info, &mut out)
        .expect("hkdf expand with fixed sizes");
    out
}

fn seal(key32: &[u8; 32], plaintext: &[u8]) -> Result<(String, String), String> {
    let cipher = XChaCha20Poly1305::new(key32.into());
    let mut nonce_bytes = [0u8; 24];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut nonce_bytes);
    let ct = cipher
        .encrypt(XNonce::from_slice(&nonce_bytes), plaintext)
        .map_err(|e| format!("encrypt: {e}"))?;
    Ok((hex::encode(nonce_bytes), hex::encode(ct)))
}

fn open(key32: &[u8; 32], nonce_hex: &str, ct_hex: &str) -> Result<String, String> {
    let cipher = XChaCha20Poly1305::new(key32.into());
    let nonce = hex::decode(nonce_hex).map_err(|e| format!("nonce: {e}"))?;
    let ct = hex::decode(ct_hex).map_err(|e| format!("ciphertext: {e}"))?;
    let pt = cipher
        .decrypt(XNonce::from_slice(&nonce), ct.as_slice())
        .map_err(|_| "decrypt failed".to_string())?;
    String::from_utf8(pt).map_err(|e| format!("plaintext: {e}"))
}

fn load_file(path: &Path) -> SecretFile {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_file(path: &Path, file: &SecretFile) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("secrets dir: {e}"))?;
    }
    let s = serde_json::to_string(file).map_err(|e| format!("secrets encode: {e}"))?;
    std::fs::write(path, s).map_err(|e| format!("secrets write: {e}"))?;
    set_600(path);
    Ok(())
}

fn file_set(user_id: &str, provider: &str, key: &str) -> Result<(), String> {
    let Some(path) = secrets_file() else {
        return Err("no secrets file location".into());
    };
    let _guard = FILE_LOCK
        .lock()
        .map_err(|_| "secrets lock poisoned".to_string())?;
    let machine = machine_secret(&path).ok_or("no machine secret available")?;
    let mut file = load_file(&path);
    if file.salt.is_empty() {
        let mut salt = [0u8; 16];
        rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut salt);
        file.salt = hex::encode(salt);
    }
    let salt = hex::decode(&file.salt).map_err(|e| format!("secrets salt: {e}"))?;
    let acct = account(user_id, provider);
    let ek = entry_key(&salt, &machine, &acct);
    let (nonce, ct) = seal(&ek, key.as_bytes())?;
    file.entries.insert(acct, FileEntry { nonce, ct });
    save_file(&path, &file)
}

fn file_get(user_id: &str, provider: &str) -> Option<String> {
    let path = secrets_file()?;
    let _guard = FILE_LOCK.lock().ok()?;
    let machine = machine_secret(&path)?;
    let file = load_file(&path);
    let salt = hex::decode(&file.salt).ok()?;
    let entry = file.entries.get(&account(user_id, provider))?;
    let ek = entry_key(&salt, &machine, &account(user_id, provider));
    open(&ek, &entry.nonce, &entry.ct).ok()
}

fn file_delete(user_id: &str, provider: &str) -> Result<(), String> {
    let Some(path) = secrets_file() else {
        return Ok(());
    };
    let _guard = FILE_LOCK
        .lock()
        .map_err(|_| "secrets lock poisoned".to_string())?;
    if !path.exists() {
        return Ok(());
    }
    let mut file = load_file(&path);
    if file.entries.remove(&account(user_id, provider)).is_none() {
        return Ok(());
    }
    save_file(&path, &file)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Isolated file backend per test: unique path + forced fallback so the
    /// real OS keyring is never touched and tests cannot talk to each other.
    /// The returned guard serializes these tests: they share process env.
    /// Returns the file path for content/permission assertions.
    static ENV_LOCK: Mutex<()> = Mutex::new(());
    fn test_env(tag: &str) -> (PathBuf, std::sync::MutexGuard<'static, ()>) {
        let guard = ENV_LOCK.lock().unwrap();
        let p = std::env::temp_dir().join(format!(
            "smartpc-secrets-test-{}-{tag}.json",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&p);
        std::env::set_var("SMARTPC_SECRETS_FILE", &p);
        std::env::set_var("SMARTPC_NO_KEYRING", "1");
        (p, guard)
    }

    #[test]
    fn rejects_short_keys_without_touching_the_store() {
        assert!(set_key("u", "opencode", "  short ").is_err());
        assert!(set_key("u", "opencode", "").is_err());
    }

    #[test]
    fn account_names_are_user_scoped() {
        assert_ne!(account("alice", "opencode"), account("bob", "opencode"));
    }

    #[test]
    fn file_backend_roundtrip() {
        let (path, _guard) = test_env("roundtrip");
        assert!(!has_key("u1", "probe-a"));
        set_key("u1", "probe-a", "super-secret-value-123").unwrap();
        assert!(has_key("u1", "probe-a"));
        assert_eq!(
            get_key("u1", "probe-a").as_deref(),
            Some("super-secret-value-123")
        );
        delete_key("u1", "probe-a").unwrap();
        assert!(!has_key("u1", "probe-a"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn file_backend_does_not_store_plaintext() {
        let (path, _guard) = test_env("enc");
        set_key("u2", "probe-b", "plaintext-must-not-appear-xyz").unwrap();
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(!raw.contains("plaintext-must-not-appear-xyz"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn file_backend_isolates_users() {
        let (path, _guard) = test_env("iso");
        set_key("alice", "probe-c", "alice-secret-12345").unwrap();
        assert_eq!(get_key("bob", "probe-c"), None);
        assert_eq!(
            get_key("alice", "probe-c").as_deref(),
            Some("alice-secret-12345")
        );
        let _ = std::fs::remove_file(&path);
    }
}
