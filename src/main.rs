//! # Gleam Package Manager
//!
//! `gleam-pkg` is a command-line tool for managing Gleam CLI programs. It provides an easy way to install and manage Gleam packages.
//!
//! wheatfox(enkerewpo@hotmail.com) 2024
//!
//! ## Features
//! - Install Gleam CLI packages
//! - Maintain a local package database
//! - Ensure proper directory structure for managing Gleam packages
//!
//! ## Usage
//!
//! Run the `gleam-pkg` CLI with the desired subcommand:
//!
//! ```sh
//! gleam-pkg install <package-name>
//! ```

use clap::{Parser, Subcommand};
use std::fs;
use std::path::PathBuf;
use error::*;

mod error;

/// Command-line interface for `gleam-pkg`
#[derive(Parser)]
#[command(name = "gleam-pkg")]
#[command(about = "Gleam package manager for installing Gleam CLI programs")]
struct Cli {
  /// The subcommand to execute
  #[command(subcommand)]
  command: Commands,
}

/// Subcommands supported by `gleam-pkg`
#[derive(Subcommand)]
enum Commands {
  /// Install a Gleam package
  Install {
    /// The name of the package to install
    package: String,
  },
}

const ROOT_DIR: &str = ".gleam_pkgs";
const DOWNLOAD_DIR: &str = "download";
const APPS_DIR: &str = "apps";
const DB_DIR: &str = "db";
const DB_FILE: &str = "db/metadata.json";

/// Entry point for the Gleam package manager CLI
fn main() -> Result<(), GleamPkgError> {
  let args = Cli::parse();
  let home_dir = dirs::home_dir().ok_or_else(|| {
    GleamPkgError::DirectoryCreationError("Unable to locate home directory".to_string())
  })?;
  let root_dir = home_dir.join(ROOT_DIR);
  setup_directories(&root_dir)?;
  match args.command {
    Commands::Install { package } => {
      install_package(&root_dir, &package)?;
    }
  }

  Ok(())
}

/// Sets up the necessary directory structure for Gleam packages
///
/// # Arguments
///
/// * `root_dir` - The root directory where Gleam packages and metadata will be stored
///
/// # Errors
///
/// Returns `GleamPkgError::DirectoryCreationError` if any of the directories cannot be created
fn setup_directories(root_dir: &PathBuf) -> Result<(), GleamPkgError> {
  let paths = [
    root_dir.to_path_buf(),
    root_dir.join(DOWNLOAD_DIR),
    root_dir.join(APPS_DIR),
    root_dir.join(DB_DIR),
  ];
  for path in paths {
    if !path.exists() {
      fs::create_dir_all(&path)
        .map_err(|e| GleamPkgError::DirectoryCreationError(format!("{path:?}: {e}")))?;
    }
  }
  Ok(())
}

/// Installs a Gleam package
///
/// # Arguments
///
/// * `root_dir` - The root directory where packages and metadata are stored
/// * `package` - The name of the package to install
///
/// # Errors
///
/// Returns `GleamPkgError` if the installation fails
///
/// # Notes
///
/// TODO: Implement this function
fn install_package(root_dir: &PathBuf, package: &str) -> Result<(), GleamPkgError> {
  let download_dir = root_dir.join(DOWNLOAD_DIR);
  let apps_dir = root_dir.join(APPS_DIR);
  let metadata_file = root_dir.join(DB_FILE);
  todo!();
  Ok(())
}
