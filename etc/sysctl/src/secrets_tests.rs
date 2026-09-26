use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn references_round_trip_without_exposing_values_or_resolving_instructions() -> anyhow::Result<()> {
    let _store = chaos_keyring::tests::MockKeyringStore::default();
    let original = serde_json::json!({
        "bearer_token":"private-token",
        "env":{"TOKEN":"private-env"},
        "http_headers":{"X-Token":"private-header"},
        "nested":[{"api_key":"private-array-key"}],
        "instructions":"keyring:chaos-settings/not-a-reference-to-resolve"
    });
    let mut stored = original.clone();
    transform(&mut stored, true)?;
    let encoded = serde_json::to_string(&stored)?;
    assert!(!encoded.contains("private-token"));
    assert!(!encoded.contains("private-env"));
    assert!(!encoded.contains("private-header"));
    assert!(!encoded.contains("private-array-key"));
    let references = stored.clone();
    transform(&mut stored, true)?;
    assert_eq!(stored, references);
    transform(&mut stored, false)?;
    assert_eq!(stored, original);
    assert!(resolve("keyring:chaos-settings/00000000-0000-0000-0000-000000000000").is_err());
    assert_eq!(
        externalize("keyring:chaos-settings/00000000-0000-0000-0000-000000000000")?,
        "keyring:chaos-settings/00000000-0000-0000-0000-000000000000"
    );
    assert!(externalize("keyring:chaos-settings/not-a-uuid").is_err());
    let reference = references["bearer_token"].as_str().unwrap();
    // Verified saves warm the cache; later reads must not hit the store.
    DefaultKeyringStore.delete(SERVICE, reference.strip_prefix(PREFIX).unwrap())?;
    assert_eq!(resolve(reference)?, "private-token");
    remove(reference)?;
    assert!(resolve(reference).is_err());
    assert!(remove("not-a-reference").is_err());
    Ok(())
}

#[test]
fn repeated_and_concurrent_reads_load_a_credential_once() {
    let cache = CredentialCache::default();
    let reads = AtomicUsize::new(0);
    let start = std::sync::Barrier::new(8);
    std::thread::scope(|scope| {
        for _ in 0..8 {
            scope.spawn(|| {
                start.wait();
                for _ in 0..3 {
                    assert_eq!(
                        cache
                            .resolve("saved-key", || {
                                reads.fetch_add(1, Ordering::SeqCst);
                                Ok("test-secret".into())
                            })
                            .unwrap(),
                        "test-secret"
                    );
                }
            });
        }
    });
    assert_eq!(reads.load(Ordering::SeqCst), 1);
}

#[test]
fn failed_reads_can_be_retried_and_new_references_do_not_reuse_old_keys() {
    let cache = CredentialCache::default();
    assert!(
        cache
            .resolve("old", || anyhow::bail!("keychain locked"))
            .is_err()
    );
    assert!(
        cache
            .resolve("old", || None::<String>.context("missing key"))
            .is_err()
    );
    assert_eq!(
        cache.resolve("old", || Ok("original".into())).unwrap(),
        "original"
    );
    assert_eq!(
        cache.resolve("new", || Ok("replacement".into())).unwrap(),
        "replacement"
    );
    assert_eq!(
        cache
            .resolve("old", || anyhow::bail!("must not reload"))
            .unwrap(),
        "original"
    );
}

#[test]
fn removal_evicts_cached_credentials_even_if_the_store_delete_fails() {
    for fail in [false, true] {
        let cache = CredentialCache::default();
        cache.resolve("key", || Ok("secret".into())).unwrap();
        let result = cache.remove("key", || {
            if fail {
                anyhow::bail!("keychain locked");
            }
            Ok(())
        });
        assert_eq!(result.is_err(), fail);
        assert!(
            cache
                .resolve("key", || anyhow::bail!("must reload"))
                .is_err()
        );
    }
}

#[test]
fn removal_waits_for_an_in_flight_read_and_does_not_leave_a_stale_value() {
    let cache = CredentialCache::default();
    let (loaded_tx, loaded_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let (removing_tx, removing_rx) = std::sync::mpsc::channel();
    std::thread::scope(|scope| {
        let cache = &cache;
        let reader = scope.spawn(move || {
            cache.resolve("key", || {
                loaded_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                Ok("secret".into())
            })
        });
        loaded_rx.recv().unwrap();
        let remover = scope.spawn(move || {
            removing_tx.send(()).unwrap();
            cache.remove("key", || Ok(()))
        });
        removing_rx.recv().unwrap();
        release_tx.send(()).unwrap();
        reader.join().unwrap().unwrap();
        remover.join().unwrap().unwrap();
    });
    assert!(
        cache
            .resolve("key", || anyhow::bail!("deleted key"))
            .is_err()
    );
}
