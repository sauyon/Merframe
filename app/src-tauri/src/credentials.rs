use anyhow::{Context, anyhow};
use keyring_core::api::CredentialStoreApi;
use keyring_core::{Entry, Error};

const SERVICE: &str = "net.yareli.merframe";
const ACCOUNT: &str = "warframe.market";

pub fn entry() -> anyhow::Result<Entry> {
    entry_for(SERVICE, ACCOUNT)
}

fn entry_for(service: &str, account: &str) -> anyhow::Result<Entry> {
    #[cfg(windows)]
    let store = windows_native_keyring_store::Store::new();
    #[cfg(target_os = "linux")]
    let store = dbus_secret_service_keyring_store::Store::new();
    #[cfg(any(windows, target_os = "linux"))]
    {
        let store = store.map_err(|_| unavailable())?;
        // Build directly against the native store: never use the library's mock
        // or sample store as a production fallback.
        #[cfg(windows)]
        let modifiers = Some(std::collections::HashMap::from([("persistence", "Local")]));
        #[cfg(target_os = "linux")]
        let modifiers = None;
        store
            .build(service, account, modifiers.as_ref())
            .map_err(|_| unavailable())
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    Err(unavailable())
}

fn unavailable() -> anyhow::Error {
    // Some backend errors contain secret bytes; do not propagate their Debug
    // values or cause chains to logs or the frontend.
    anyhow!(
        "The OS credential store is unavailable. Unlock Windows Credential Manager or your Linux Secret Service and try again."
    )
}

pub fn read(entry: &Entry) -> anyhow::Result<Option<String>> {
    match entry.get_password() {
        Ok(token) => Ok(Some(token)),
        Err(Error::NoEntry) => Ok(None),
        Err(_) => Err(unavailable()),
    }
}

pub fn write(entry: &Entry, token: &str) -> anyhow::Result<()> {
    entry.set_password(token).map_err(|_| unavailable())
}

pub fn delete(entry: &Entry) -> anyhow::Result<()> {
    match entry.delete_credential() {
        Ok(()) | Err(Error::NoEntry) => Ok(()),
        Err(_) => Err(unavailable()),
    }
}

pub fn migrate(
    entry: &Entry,
    legacy: impl FnOnce() -> anyhow::Result<Option<String>>,
    cleanup: impl FnOnce() -> anyhow::Result<()>,
) -> anyhow::Result<Option<String>> {
    if let Some(token) = read(entry)? {
        cleanup().context("Removing obsolete plaintext session copies")?;
        return Ok(Some(token));
    }
    let Some(token) = legacy()? else {
        return Ok(None);
    };
    write(entry, &token)?;
    if read(entry)?.as_deref() != Some(token.as_str()) {
        return Err(anyhow!(
            "The OS credential store did not retain the session"
        ));
    }
    // Only remove the old copy after a verified successful write. A locked
    // keyring must never silently discard an existing session.
    cleanup().context("Removing the migrated plaintext session")?;
    Ok(Some(token))
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    fn mock_entry() -> Entry {
        keyring_core::mock::Store::new()
            .unwrap()
            .build(SERVICE, ACCOUNT, None)
            .unwrap()
    }

    #[test]
    fn migration_persists_before_removing_plaintext() {
        let entry = mock_entry();
        let cleaned = Cell::new(false);
        let token = migrate(
            &entry,
            || Ok(Some("legacy-session".to_owned())),
            || {
                assert_eq!(read(&entry)?.as_deref(), Some("legacy-session"));
                cleaned.set(true);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(token.as_deref(), Some("legacy-session"));
        assert!(cleaned.get());
        delete(&entry).unwrap();
        assert_eq!(read(&entry).unwrap(), None);
        delete(&entry).unwrap();
    }

    #[test]
    fn existing_secure_session_wins_and_stale_copies_are_removed() {
        let entry = mock_entry();
        write(&entry, "current-session").unwrap();
        let cleaned = Cell::new(false);
        let token = migrate(
            &entry,
            || panic!("An obsolete plaintext session must not be read"),
            || {
                cleaned.set(true);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(token.as_deref(), Some("current-session"));
        assert!(cleaned.get());
    }

    #[test]
    fn failed_write_preserves_the_legacy_copy() {
        let entry = mock_entry();
        let cleaned = Cell::new(false);
        let result = migrate(
            &entry,
            || {
                entry
                    .as_any()
                    .downcast_ref::<keyring_core::mock::Cred>()
                    .unwrap()
                    .set_error(Error::BadEncoding(b"secret-do-not-log".to_vec()));
                Ok(Some("legacy-session".to_owned()))
            },
            || {
                cleaned.set(true);
                Ok(())
            },
        );
        assert!(result.is_err());
        assert!(!cleaned.get());
        assert!(!format!("{:#}", result.unwrap_err()).contains("secret-do-not-log"));
        assert_eq!(read(&entry).unwrap(), None);
    }

    #[test]
    #[ignore = "Requires the current user's unlocked native credential store"]
    fn native_credential_store_round_trip() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let entry = entry_for("net.yareli.merframe.integration-test", &unique.to_string()).unwrap();
        write(&entry, "temporary-test-session").unwrap();
        let saved = read(&entry);
        let removed = delete(&entry);
        assert_eq!(saved.unwrap().as_deref(), Some("temporary-test-session"));
        removed.unwrap();
        assert_eq!(read(&entry).unwrap(), None);
    }

    #[test]
    fn cleanup_failure_can_be_retried_without_overwriting_secure_session() {
        let entry = mock_entry();
        assert!(
            migrate(
                &entry,
                || Ok(Some("session".to_owned())),
                || Err(anyhow!("simulated file permission failure")),
            )
            .is_err()
        );
        assert_eq!(read(&entry).unwrap().as_deref(), Some("session"));
        assert_eq!(
            migrate(&entry, || Ok(None), || Ok(())).unwrap().as_deref(),
            Some("session")
        );
    }
}
