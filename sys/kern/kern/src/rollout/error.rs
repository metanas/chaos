use std::io::ErrorKind;
use std::path::Path;

use crate::error::ChaosErr;

pub(crate) fn map_session_init_error(err: &anyhow::Error, chaos_home: &Path) -> ChaosErr {
    for cause in err.chain() {
        let kernel_error = cause.downcast_ref::<ChaosErr>().or_else(|| {
            cause
                .downcast_ref::<std::io::Error>()?
                .get_ref()?
                .downcast_ref::<ChaosErr>()
        });
        if let Some(ChaosErr::SessionInUse(process_id)) = kernel_error {
            return ChaosErr::SessionInUse(*process_id);
        }
    }
    match diagnose_session_init_error(err, chaos_home) {
        Some(message) => ChaosErr::Fatal(message),
        None => ChaosErr::Fatal(format!("Failed to initialize session: {err:#}")),
    }
}

fn diagnose_session_init_error(err: &anyhow::Error, chaos_home: &Path) -> Option<String> {
    err.chain()
        .filter_map(|cause| cause.downcast_ref::<std::io::Error>())
        .find_map(|io_err| diagnose_io_error(io_err, chaos_home))
}

fn diagnose_io_error(io_err: &std::io::Error, chaos_home: &Path) -> Option<String> {
    let hint = match io_err.kind() {
        ErrorKind::PermissionDenied => format!(
            "ChaOS cannot access persisted session storage under {} (permission denied). \
             If session state was created using sudo, fix ownership: \
             sudo chown -R $(whoami) {}",
            chaos_home.display(),
            chaos_home.display()
        ),
        ErrorKind::NotFound => format!(
            "Persisted session storage is missing under {}. \
             Create the directory or choose a different ChaOS home.",
            chaos_home.display()
        ),
        ErrorKind::AlreadyExists => format!(
            "A required session-storage path under {} is blocked by an existing file. \
             Remove or rename it so ChaOS can continue.",
            chaos_home.display()
        ),
        ErrorKind::InvalidData | ErrorKind::InvalidInput => format!(
            "Persisted session state under {} looks corrupt or unreadable.",
            chaos_home.display()
        ),
        ErrorKind::IsADirectory | ErrorKind::NotADirectory => format!(
            "A persisted-session storage path under {} has an unexpected type.",
            chaos_home.display()
        ),
        _ => return None,
    };

    Some(format!("{hint} (underlying error: {io_err})"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chaos_ipc::ProcessId;

    #[test]
    fn session_in_use_survives_init_context_and_io_wrapping() {
        let process_id = ProcessId::new();
        let error = anyhow::Error::new(std::io::Error::other(ChaosErr::SessionInUse(process_id)))
            .context("failed to create rollout recorder");
        let mapped = map_session_init_error(&error, Path::new("/unused"));
        std::assert_matches!(mapped, ChaosErr::SessionInUse(id) if id == process_id);
        assert!(!mapped.is_retryable());
    }

    #[test]
    fn session_init_does_not_classify_errors_by_message_text() {
        let error = anyhow::anyhow!("LeaseConflict: unrelated storage failure")
            .context("failed to create rollout recorder");
        let mapped = map_session_init_error(&error, Path::new("/unused"));
        std::assert_matches!(mapped, ChaosErr::Fatal(_));
        assert!(mapped.to_string().contains("unrelated storage failure"));
    }
}
