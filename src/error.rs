use thiserror::Error;

/// Custom error type for the `gleam-pkg` package manager
///
/// This enum defines all possible errors that may occur while using the Gleam package manager.
/// Each variant includes a message describing the specific error.
#[derive(Error, Debug)]
pub enum GleamPkgError {
    /// Error indicating a failure to start a program
    ///
    /// # Example
    /// This error might occur if a required Gleam program cannot be executed.
    #[error("Failed to start program: {0}")]
    ProgramError(String),

    /// Error indicating a failure to create necessary directories
    ///
    /// # Example
    /// This error might occur if there are insufficient permissions to create directories.
    #[error("Failed to create directories: {0}")]
    DirectoryCreationError(String),

    #[error("Failed to download package: {0}")]
    PackageDownloadError(String),
}
