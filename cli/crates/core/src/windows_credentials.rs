//! Windows generic credentials hold at most 2,560 bytes. The keyring backend
//! encodes passwords as UTF-16, so full OAuth JSON needs multiple entries.
//! Publish a new generation only after all its chunks have been saved.
use super::{Result, MAX_AUTH_BYTES};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const MAX_UTF16_UNITS: usize = 1280;
const REFERENCE_PREFIX: &str = "multi-codex-chunks-v1:";
const SAVE_ERROR: &str = "Windows Credential Manager could not save this account";
const READ_ERROR: &str = "Windows Credential Manager could not read this account";
const DELETE_ERROR: &str = "Windows Credential Manager could not delete this account";

trait Backend {
    fn read(&self, key: &str) -> keyring::Result<String>;
    fn write(&self, key: &str, value: &str) -> keyring::Result<()>;
    fn delete(&self, key: &str) -> keyring::Result<()>;
}

#[cfg(windows)]
struct NativeBackend;

#[cfg(windows)]
impl Backend for NativeBackend {
    fn read(&self, key: &str) -> keyring::Result<String> {
        keyring::Entry::new(super::KEYRING_SERVICE, key)?.get_password()
    }
    fn write(&self, key: &str, value: &str) -> keyring::Result<()> {
        keyring::Entry::new(super::KEYRING_SERVICE, key)?.set_password(value)
    }
    fn delete(&self, key: &str) -> keyring::Result<()> {
        keyring::Entry::new(super::KEYRING_SERVICE, key)?.delete_credential()
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Reference {
    generation: Uuid,
    chunks: usize,
}

impl Reference {
    fn key(&self, id: &str, index: usize) -> String {
        format!("{id}:chunk:{}:{index}", self.generation)
    }
}

fn parse_reference(value: &str) -> Result<Option<Reference>> {
    let Some(json) = value.strip_prefix(REFERENCE_PREFIX) else {
        return Ok(None);
    };
    let reference: Reference = serde_json::from_str(json)
        .map_err(|_| "The saved Windows credential reference is invalid".to_string())?;
    if reference.chunks == 0 || reference.chunks > MAX_AUTH_BYTES.div_ceil(MAX_UTF16_UNITS - 1) {
        return Err("The saved Windows credential reference is invalid".into());
    }
    Ok(Some(reference))
}

fn split_secret(secret: &str) -> Vec<&str> {
    let mut chunks = Vec::new();
    let mut start = 0;
    let mut units = 0;
    for (index, character) in secret.char_indices() {
        if units + character.len_utf16() > MAX_UTF16_UNITS {
            chunks.push(&secret[start..index]);
            start = index;
            units = 0;
        }
        units += character.len_utf16();
    }
    chunks.push(&secret[start..]);
    chunks
}

fn delete_chunks(backend: &impl Backend, id: &str, reference: &Reference) -> Result<()> {
    let mut failed = false;
    for index in 0..reference.chunks {
        match backend.delete(&reference.key(id, index)) {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(_) => failed = true,
        }
    }
    if failed {
        Err(DELETE_ERROR.into())
    } else {
        Ok(())
    }
}

fn save_with(backend: &impl Backend, id: &str, secret: &str) -> Result<()> {
    if secret.len() > MAX_AUTH_BYTES {
        return Err("The account credential is too large".into());
    }
    let previous = match backend.read(id) {
        Ok(value) if value == secret => return Ok(()),
        Ok(value) => parse_reference(&value)?,
        Err(keyring::Error::NoEntry) => None,
        Err(_) => return Err(READ_ERROR.into()),
    };
    if previous.is_some() && load_with(backend, id).is_ok_and(|stored| stored == secret) {
        // Launches and usage checks synchronize the same credential repeatedly.
        // Reuse its existing generation instead of rewriting the whole vault.
        return Ok(());
    }
    let chunks = split_secret(secret);
    if chunks.len() == 1 {
        backend
            .write(id, secret)
            .map_err(|_| SAVE_ERROR.to_string())?;
    } else {
        let next = Reference {
            generation: Uuid::new_v4(),
            chunks: chunks.len(),
        };
        let root = format!(
            "{REFERENCE_PREFIX}{}",
            serde_json::to_string(&next).map_err(|_| SAVE_ERROR.to_string())?
        );
        for (index, chunk) in chunks.iter().enumerate() {
            if backend.write(&next.key(id, index), chunk).is_err() {
                let _ = delete_chunks(backend, id, &next);
                return Err(SAVE_ERROR.into());
            }
        }
        if backend.write(id, &root).is_err() {
            let _ = delete_chunks(backend, id, &next);
            return Err(SAVE_ERROR.into());
        }
    }
    if let Some(previous) = previous {
        // The new value is committed. A failed old-generation cleanup must not
        // report the save as failed and trigger a caller's credential rollback.
        let _ = delete_chunks(backend, id, &previous);
    }
    Ok(())
}

fn load_with(backend: &impl Backend, id: &str) -> Result<String> {
    let value = backend.read(id).map_err(|_| READ_ERROR.to_string())?;
    let Some(reference) = parse_reference(&value)? else {
        return Ok(value);
    };
    let mut secret = String::new();
    for index in 0..reference.chunks {
        let chunk = backend
            .read(&reference.key(id, index))
            .map_err(|_| READ_ERROR.to_string())?;
        if chunk.encode_utf16().count() > MAX_UTF16_UNITS
            || secret.len() + chunk.len() > MAX_AUTH_BYTES
        {
            return Err("The saved Windows credential is invalid".into());
        }
        secret.push_str(&chunk);
    }
    Ok(secret)
}

fn remove_with(backend: &impl Backend, id: &str) -> Result<()> {
    let value = match backend.read(id) {
        Ok(value) => value,
        Err(keyring::Error::NoEntry) => return Ok(()),
        Err(_) => return Err(READ_ERROR.into()),
    };
    if let Some(reference) = parse_reference(&value)? {
        // Retain the reference on a partial failure so deletion can be retried.
        delete_chunks(backend, id, &reference)?;
    }
    match backend.delete(id) {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err(DELETE_ERROR.into()),
    }
}

#[cfg(windows)]
static CREDENTIAL_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(windows)]
pub(super) fn save(id: &str, secret: &str) -> Result<()> {
    let _guard = CREDENTIAL_LOCK.lock().map_err(|_| SAVE_ERROR.to_string())?;
    save_with(&NativeBackend, id, secret)
}

#[cfg(windows)]
pub(super) fn load(id: &str) -> Result<String> {
    let _guard = CREDENTIAL_LOCK.lock().map_err(|_| READ_ERROR.to_string())?;
    load_with(&NativeBackend, id)
}

#[cfg(windows)]
pub(super) fn remove(id: &str) -> Result<()> {
    let _guard = CREDENTIAL_LOCK
        .lock()
        .map_err(|_| DELETE_ERROR.to_string())?;
    remove_with(&NativeBackend, id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    use std::collections::HashMap;

    #[derive(Default)]
    struct LimitedBackend {
        values: RefCell<HashMap<String, String>>,
        fail_write_at: Cell<Option<usize>>,
        writes: Cell<usize>,
    }

    impl Backend for LimitedBackend {
        fn read(&self, key: &str) -> keyring::Result<String> {
            self.values
                .borrow()
                .get(key)
                .cloned()
                .ok_or(keyring::Error::NoEntry)
        }
        fn write(&self, key: &str, value: &str) -> keyring::Result<()> {
            let write = self.writes.get();
            self.writes.set(write + 1);
            if self.fail_write_at.get() == Some(write) {
                return Err(keyring::Error::NoDefaultStore);
            }
            if value.encode_utf16().count() > MAX_UTF16_UNITS {
                return Err(keyring::Error::TooLong(
                    "password encoded as UTF-16".into(),
                    2560,
                ));
            }
            self.values.borrow_mut().insert(key.into(), value.into());
            Ok(())
        }
        fn delete(&self, key: &str) -> keyring::Result<()> {
            self.values
                .borrow_mut()
                .remove(key)
                .map(|_| ())
                .ok_or(keyring::Error::NoEntry)
        }
    }

    fn oauth_json() -> String {
        serde_json::json!({
            "auth_mode": "chatgpt",
            "tokens": {
                "id_token": "inert-id-token-".repeat(350),
                "access_token": "inert-access-token-".repeat(300),
                "refresh_token": "inert-refresh-token-".repeat(200),
            },
        })
        .to_string()
    }

    #[test]
    fn oauth_sized_credentials_round_trip_and_delete_without_affecting_peers() {
        let backend = LimitedBackend::default();
        let secret = oauth_json();
        assert!(
            backend.write("account", &secret).is_err(),
            "Old single-entry storage must reproduce the Windows size failure"
        );
        save_with(&backend, "peer", "peer credential").unwrap();
        save_with(&backend, "account", &secret).unwrap();
        assert_eq!(load_with(&backend, "account").unwrap(), secret);
        remove_with(&backend, "account").unwrap();
        remove_with(&backend, "account").unwrap();
        assert_eq!(backend.values.borrow().len(), 1);
        assert_eq!(load_with(&backend, "peer").unwrap(), "peer credential");
    }

    #[test]
    fn preserves_legacy_values_and_reclaims_previous_generations_on_updates() {
        let backend = LimitedBackend::default();
        backend.write("account", "legacy credential").unwrap();
        assert_eq!(load_with(&backend, "account").unwrap(), "legacy credential");
        save_with(&backend, "account", &oauth_json()).unwrap();
        let before = backend.values.borrow().clone();
        let writes = backend.writes.get();
        save_with(&backend, "account", &oauth_json()).unwrap();
        assert_eq!(*backend.values.borrow(), before);
        assert_eq!(backend.writes.get(), writes);
        let previous_keys: Vec<_> = backend
            .values
            .borrow()
            .keys()
            .filter(|key| *key != "account")
            .cloned()
            .collect();
        save_with(&backend, "account", &"replacement-".repeat(500)).unwrap();
        assert_eq!(
            load_with(&backend, "account").unwrap(),
            "replacement-".repeat(500)
        );
        assert!(previous_keys
            .iter()
            .all(|key| !backend.values.borrow().contains_key(key)));
        save_with(&backend, "account", "short replacement").unwrap();
        assert_eq!(backend.values.borrow().len(), 1);
    }

    #[test]
    fn counts_utf16_units_and_never_splits_unicode_characters() {
        let backend = LimitedBackend::default();
        for secret in [
            "a".repeat(1280),
            "a".repeat(1281),
            "a".repeat(1279) + &"🔑工具".repeat(700),
        ] {
            save_with(&backend, "account", &secret).unwrap();
            assert_eq!(load_with(&backend, "account").unwrap(), secret);
            assert!(backend
                .values
                .borrow()
                .values()
                .all(|value| value.encode_utf16().count() <= MAX_UTF16_UNITS));
        }
    }

    #[test]
    fn failed_chunk_or_root_writes_preserve_existing_credentials_and_remove_new_chunks() {
        for existing in ["legacy credential".to_string(), oauth_json()] {
            for failed_write in [0, 1, 3] {
                let backend = LimitedBackend::default();
                save_with(&backend, "account", &existing).unwrap();
                let before = backend.values.borrow().clone();
                backend.writes.set(0);
                backend.fail_write_at.set(Some(failed_write));
                // Three chunks, followed by the root reference at write 3.
                assert!(save_with(&backend, "account", &"x".repeat(3000)).is_err());
                assert_eq!(*backend.values.borrow(), before);
                assert_eq!(load_with(&backend, "account").unwrap(), existing);
            }
        }
    }

    #[test]
    fn rejects_malformed_references_and_missing_chunks_without_returning_partial_secrets() {
        let backend = LimitedBackend::default();
        backend
            .write(
                "account",
                &format!(
                    "{REFERENCE_PREFIX}{{\"generation\":\"{}\",\"chunks\":1000000}}",
                    Uuid::new_v4()
                ),
            )
            .unwrap();
        assert!(load_with(&backend, "account").is_err());
        backend.values.borrow_mut().clear();
        save_with(&backend, "account", &oauth_json()).unwrap();
        let reference = parse_reference(&backend.read("account").unwrap())
            .unwrap()
            .unwrap();
        backend.delete(&reference.key("account", 1)).unwrap();
        assert!(load_with(&backend, "account").is_err());
    }
}
