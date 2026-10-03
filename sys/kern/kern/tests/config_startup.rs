//! Application configuration must not stall unrelated async work on slow I/O.
#![cfg(unix)]

use std::ffi::CString;
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::sync::mpsc;

use anyhow::{Context, ensure};
use chaos_kern::config::{ConfigBuilder, ConfigOverrides};
use chaos_kern::user_settings;
use chaos_test_fixtures::TEST_MODEL;
use jiff::SignedDuration;
use tokio::sync::oneshot;
use tokio::time::Instant;

// Deadlock backstops, not startup-performance assertions.
const FIXTURE_DEADLINE: SignedDuration = SignedDuration::from_secs(10);
const FIXTURE_POLL_INTERVAL: SignedDuration = SignedDuration::from_millis(10);

fn builder(home: &Path) -> ConfigBuilder {
    ConfigBuilder::default()
        .chaos_home(home.to_path_buf())
        .harness_overrides(ConfigOverrides {
            cwd: Some(home.to_path_buf()),
            ..Default::default()
        })
}

#[tokio::test(flavor = "current_thread")]
async fn slow_bootstrap_read_leaves_runtime_responsive_and_loads_persisted_settings()
-> anyhow::Result<()> {
    let home = tempfile::tempdir()?;
    user_settings::set_bootstrap(
        home.path(),
        "storage_url",
        &format!("sqlite://{}", home.path().join("chaos.sqlite").display()),
    )?;
    let runtime = user_settings::open(home.path()).await?;
    let snapshot = runtime.settings_snapshot().await?;
    runtime
        .commit_settings(
            snapshot.revision,
            &serde_json::json!({"model": TEST_MODEL}),
            None,
        )
        .await?;
    // Verify the full configuration is valid before introducing slow storage.
    builder(home.path()).build().await?;

    let bootstrap = home.path().join("config.toml");
    let source = std::fs::read(&bootstrap)?;
    let replacement = home.path().join("bootstrap-ready.toml");
    std::fs::write(&replacement, &source)?;
    std::fs::remove_file(&bootstrap)?;
    let name = CString::new(bootstrap.as_os_str().as_bytes())?;
    // SAFETY: name is a live, NUL-terminated path; this creates a private fixture.
    ensure!(
        unsafe { libc::mkfifo(name.as_ptr(), 0o600) } == 0,
        "create slow bootstrap fixture: {}",
        std::io::Error::last_os_error()
    );

    let (started_tx, started_rx) = oneshot::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let writer = std::thread::spawn(move || -> anyhow::Result<bool> {
        // Wait for the application's reader without leaving a hung fixture if
        // configuration fails before it ever reads the bootstrap file.
        let start = Instant::now();
        let mut file = loop {
            match std::fs::OpenOptions::new()
                .write(true)
                .custom_flags(libc::O_NONBLOCK)
                .open(&bootstrap)
            {
                Ok(file) => break file,
                Err(error) if error.raw_os_error() == Some(libc::ENXIO) => {
                    ensure!(
                        start.elapsed() < FIXTURE_DEADLINE.unsigned_abs(),
                        "configuration never opened the slow bootstrap fixture"
                    );
                    std::thread::sleep(FIXTURE_POLL_INTERVAL.unsigned_abs());
                }
                Err(error) => return Err(error.into()),
            }
        };
        file.write_all(&source)?;
        // Later reads use a regular file; keep this first read waiting for EOF.
        std::fs::rename(replacement, bootstrap)?;
        let _ = started_tx.send(());
        let async_work_ran = release_rx
            .recv_timeout(FIXTURE_DEADLINE.unsigned_abs())
            .is_ok();
        drop(file);
        Ok(async_work_ran)
    });

    let loader = builder(home.path());
    let load = tokio::spawn(async move { loader.build().await });
    let started = tokio::time::timeout(FIXTURE_DEADLINE.unsigned_abs(), started_rx).await;
    // This async continuation must run while configuration is still waiting
    // for the fixture's EOF, not only after the external backstop releases it.
    let released = release_tx.send(()).is_ok();
    let async_work_ran = writer
        .join()
        .map_err(|_| anyhow::anyhow!("bootstrap fixture panicked"))??;
    let config = load.await??;
    started.context("bootstrap fixture did not start")??;
    ensure!(
        released && async_work_ran,
        "configuration loading stalled unrelated async work"
    );
    assert_eq!(config.model.as_deref(), Some(TEST_MODEL));
    Ok(())
}

#[tokio::test]
async fn unavailable_provider_credentials_still_fail_configuration_closed() -> anyhow::Result<()> {
    let home = tempfile::tempdir()?;
    user_settings::set_bootstrap(
        home.path(),
        "storage_url",
        &format!("sqlite://{}", home.path().join("chaos.sqlite").display()),
    )?;
    let runtime = user_settings::open(home.path()).await?;
    let snapshot = runtime.settings_snapshot().await?;
    let mut settings = serde_json::json!({
        "model_provider": "fixture",
        "model": TEST_MODEL,
        "model_providers": {"fixture": {"name": "Fixture", "wire_api": "responses"}},
    });
    runtime
        .commit_settings(snapshot.revision, &settings, None)
        .await?;
    // Establish that this provider/configuration is valid without credentials.
    let config = builder(home.path()).build().await?;
    assert_eq!(config.model_provider_id, "fixture");

    settings["model_providers"]["fixture"]["experimental_bearer_token"] =
        serde_json::json!(format!("keyring:chaos-settings/{}", uuid::Uuid::new_v4()));
    let snapshot = runtime.settings_snapshot().await?;
    runtime
        .commit_settings(snapshot.revision, &settings, None)
        .await?;
    // No vault/keyring is used: the reference deliberately names an unavailable
    // credential in this isolated home. Loading must not silently use defaults.
    ensure!(
        builder(home.path()).build().await.is_err(),
        "configuration must reject unavailable provider credentials"
    );
    assert_eq!(runtime.settings_snapshot().await?.settings, settings);
    Ok(())
}
