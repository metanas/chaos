#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("not a git repository: {0}")]
    NotARepo(String),

    #[error("git operation failed: {0}")]
    Operation(String),

    #[error("reference not found: {0}")]
    RefNotFound(String),

    #[error("invalid git input: {0}")]
    InvalidInput(String),

    #[error("path has unresolved index conflicts: {0}")]
    Conflict(String),

    #[error("git diff exceeded its safe work limit: {0}")]
    DiffLimit(String),

    #[error("git operation cancelled")]
    Cancelled,
}

impl From<gix::Error> for GitError {
    fn from(error: gix::Error) -> Self {
        if error.is_not_found() {
            Self::RefNotFound(error.to_string())
        } else if error.is_validation() {
            Self::InvalidInput(error.to_string())
        } else {
            Self::Operation(error.to_string())
        }
    }
}
