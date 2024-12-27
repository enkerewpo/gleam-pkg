//! # Gleam Package Manager
//!
//! `gleam-pkg` is a command-line tool for managing Gleam CLI programs. It provides an easy way to
//! install and manage Gleam packages. Some functionality are still under development.
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
use error::*;
use flate2::read::GzDecoder;
use lazy_static::lazy_static;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

mod error;

/// Command-line interface for `gleam-pkg`
#[derive(Parser)]
#[command(name = "gleam-pkg")]
#[command(about = "Gleam package manager for installing Gleam CLI programs")]
#[command(arg_required_else_help = true)]
struct Cli {
    /// The version of the Gleam package manager
    #[arg(
        short = 'v',
        long = "version",
        help = "Prints the version of the Gleam package manager"
    )]
    version: bool,
    /// The subcommand to execute
    #[command(subcommand)]
    command: Option<Commands>,
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

/// Configuration for the Gleam package manager
struct Config {
    api_base: String,
    repository_base: String,
}

impl Config {
    fn new() -> Self {
        Config {
            api_base: "https://hex.pm/api/".to_string(),
            repository_base: "https://repo.hex.pm/".to_string(),
        }
    }
}

// lazyinit a Config
lazy_static! {
    static ref CONFIG: Config = Config::new();
}

lazy_static! {
    static ref HOME_ROOT_DIR: PathBuf = dirs::home_dir().unwrap().join(ROOT_DIR);
}

/// Entry point for the Gleam package manager CLI
fn main() -> Result<(), GleamPkgError> {
    let args = Cli::parse();

    if args.version {
        println!("Gleam Package Manager v{}", env!("CARGO_PKG_VERSION"));
        println!("Software published under {}", env!("CARGO_PKG_LICENSE"));
        println!("Author: {}", env!("CARGO_PKG_AUTHORS"));
        return Ok(());
    }

    let home_dir = dirs::home_dir().ok_or_else(|| {
        GleamPkgError::DirectoryCreationError("Unable to locate home directory".to_string())
    })?;
    let root_dir = home_dir.join(ROOT_DIR);
    setup_directories(&root_dir)?;
    match args.command {
        Some(Commands::Install { package }) => {
            let home_dir = dirs::home_dir().ok_or_else(|| {
                GleamPkgError::DirectoryCreationError("Unable to locate home directory".to_string())
            })?;
            let root_dir = home_dir.join(ROOT_DIR);
            setup_directories(&root_dir)?;
            install_package(&root_dir, &package)?;
        }
        None => {
            println!("No subcommand provided. Use `gleam-pkg --help` for usage information.");
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
fn install_package(root_dir: &PathBuf, package: &str) -> Result<(), GleamPkgError> {
    let download_dir = root_dir.join(DOWNLOAD_DIR);

    let metadata = fetch_metadata(package)?;
    let version = extract_version(&metadata)?;
    let tarball = download_tarball(package, &version)?;

    save_tarball(&download_dir, package, &version, tarball)?;
    extract(&download_dir, package, &version)?;
    build_package(&download_dir, package, &version)?;

    Ok(())
}

fn fetch_metadata(package: &str) -> Result<serde_json::Value, GleamPkgError> {
    let client = reqwest::blocking::Client::new();
    let url = format!("{}packages/{}", CONFIG.api_base, package);
    println!("Inspecting package from: {}", url);

    let response = client
        .get(&url)
        .header("accept", "application/json")
        .header("user-agent", "gleam-pkg")
        .send()
        .map_err(|e| {
            GleamPkgError::PackageDownloadError(format!(
                "Failed to fetch metadata for package: {}, {}",
                package, e
            ))
        })?;

    if !response.status().is_success() {
        return Err(GleamPkgError::PackageDownloadError(format!(
            "Received non-success status code: {}",
            response.status()
        )));
    }

    response.json::<serde_json::Value>().map_err(|e| {
        GleamPkgError::PackageDownloadError(format!(
            "Returned metadata is not valid JSON: {}, {}",
            package, e
        ))
    })
}

/// Extracts the version of a package from its metadata
///
/// # Arguments
///
/// * `metadata` - The metadata of the package
///
/// # Errors
///
/// Returns `GleamPkgError` if the version cannot be extracted
///
fn extract_version(metadata: &serde_json::Value) -> Result<String, GleamPkgError> {
    let releases = metadata["releases"].as_array().ok_or_else(|| {
        GleamPkgError::PackageDownloadError("No releases found in metadata".to_string())
    })?;

    releases[0]["version"]
        .as_str()
        .map(String::from)
        .ok_or_else(|| {
            GleamPkgError::PackageDownloadError("No version found in metadata".to_string())
        })
}

/// Downloads a tarball of a package
///
/// # Arguments
///
/// * `package` - The name of the package to download
/// * `version` - The version of the package to download
///
/// # Errors
///
/// Returns `GleamPkgError` if the download fails
///
/// # Returns
///
/// The tarball as a byte array
///
fn download_tarball(package: &str, version: &str) -> Result<bytes::Bytes, GleamPkgError> {
    let client = reqwest::blocking::Client::new();
    let url = format!(
        "{}tarballs/{}-{}.tar",
        CONFIG.repository_base, package, version
    );
    println!("Downloading package from: {}", url);

    let response = client
        .get(&url)
        .header("accept", "application/x-tar")
        .header("user-agent", "gleam-pkg")
        .send()
        .map_err(|e| {
            GleamPkgError::PackageDownloadError(format!(
                "Failed to download package: {}, {}",
                package, e
            ))
        })?;

    if !response.status().is_success() {
        return Err(GleamPkgError::PackageDownloadError(format!(
            "Received non-success status code: {}",
            response.status()
        )));
    }

    response.bytes().map_err(|e| {
        GleamPkgError::PackageDownloadError(format!("Failed to read tarball bytes: {}", e))
    })
}

/// Saves a tarball to disk
///
/// # Arguments
///
/// * `download_dir` - The directory where the tarball will be saved
/// * `package` - The name of the package
/// * `version` - The version of the package
/// * `tarball` - The tarball as a byte array
///
/// # Errors
///
/// Returns `GleamPkgError` if the tarball cannot be saved
///
fn save_tarball(
    download_dir: &PathBuf,
    package: &str,
    version: &str,
    tarball: bytes::Bytes,
) -> Result<(), GleamPkgError> {
    let tarball_path = download_dir.join(format!("{}-{}.tar", package, version));
    fs::write(&tarball_path, tarball).map_err(|e| {
        GleamPkgError::PackageDownloadError(format!(
            "Failed to save tarball to disk: {}, {}",
            tarball_path.display(),
            e
        ))
    })?;
    println!("Tarball saved to: {}", tarball_path.display());
    Ok(())
}

/// Extracts a tarball to disk
///
/// # Arguments
///
/// * `download_dir` - The directory where the tarball is saved
/// * `package` - The name of the package
/// * `version` - The version of the package
///
/// # Errors
///
/// Returns `GleamPkgError` if the tarball cannot be extracted
///
fn extract(download_dir: &PathBuf, package: &str, version: &str) -> Result<(), GleamPkgError> {
    let tarball_path = download_dir.join(format!("{}-{}.tar", package, version));
    let extract_dir = download_dir.join(format!("{}-{}", package, version));

    // if extract_dir exists, remove it
    if extract_dir.exists() {
        fs::remove_dir_all(&extract_dir).map_err(|e| {
            GleamPkgError::PackageDownloadError(format!(
                "Failed to remove existing extract directory: {}, {}",
                extract_dir.display(),
                e
            ))
        })?;
    }
    fs::create_dir(&extract_dir).map_err(|e| {
        GleamPkgError::PackageDownloadError(format!(
            "Failed to create extract directory: {}, {}",
            extract_dir.display(),
            e
        ))
    })?;

    let tar = fs::File::open(&tarball_path).map_err(|e| {
        GleamPkgError::PackageDownloadError(format!(
            "Failed to open tarball for extraction: {}, {}",
            tarball_path.display(),
            e
        ))
    })?;
    let mut archive = tar::Archive::new(tar);
    archive.unpack(&extract_dir).map_err(|e| {
        GleamPkgError::PackageDownloadError(format!(
            "Failed to extract tarball: {}, {}",
            tarball_path.display(),
            e
        ))
    })?;
    println!("Tarball extracted to: {}", extract_dir.display());
    // then enter the extracted directory and extract contents.tar.gz to contents
    let contents_tar_gz = extract_dir.join("contents.tar.gz");
    let contents_dir = extract_dir.join("contents");
    let contents_tar = fs::File::open(&contents_tar_gz).map_err(|e| {
        GleamPkgError::PackageDownloadError(format!(
            "Failed to open contents tarball for extraction: {}, {}",
            contents_tar_gz.display(),
            e
        ))
    })?;
    let decoder = GzDecoder::new(contents_tar);
    let mut contents_tar = tar::Archive::new(decoder);
    contents_tar.unpack(&contents_dir).map_err(|e| {
        GleamPkgError::PackageDownloadError(format!(
            "Failed to extract contents tarball: {}, {}",
            contents_tar_gz.display(),
            e
        ))
    })?;
    println!("Contents extracted to: {}", contents_dir.display());
    Ok(())
}

/// Builds a package
/// This involves running `gleam build` and `gleam export erlang-shipment` in the contents directory
///
/// # Arguments
///
/// * `download_dir` - The directory where the package is downloaded
/// * `package` - The name of the package
/// * `version` - The version of the package
///
/// # Errors
///
/// Returns `GleamPkgError` if the package cannot be built
fn build_package(
    download_dir: &PathBuf,
    package: &str,
    version: &str,
) -> Result<(), GleamPkgError> {
    // run `gleam build` in contents directory
    let contents_dir = download_dir.join(format!("{}-{}/contents", package, version));
    let output = std::process::Command::new("gleam")
        .arg("build")
        .current_dir(&contents_dir)
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .output()
        .map_err(|e| {
            GleamPkgError::PackageBuildError(format!(
                "Failed to run `gleam build` in contents directory: {}, {}",
                contents_dir.display(),
                e
            ))
        })?;
    if !output.status.success() {
        return Err(GleamPkgError::PackageBuildError(format!(
            "Failed to build package: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    // then run `gleam export erlang-shipment`
    let output = std::process::Command::new("gleam")
        .arg("export")
        .arg("erlang-shipment")
        .current_dir(&contents_dir)
        .stderr(std::process::Stdio::inherit())
        .output()
        .map_err(|e| {
            GleamPkgError::PackageBuildError(format!(
                "Failed to run `gleam export erlang-shipment` in contents directory: {}, {}",
                contents_dir.display(),
                e
            ))
        })?;
    if !output.status.success() {
        return Err(GleamPkgError::PackageBuildError(format!(
            "Failed to export package: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    println!("Package built successfully, installing to apps directory");

    // first remove the existing ~/.gleam_pkgs/apps/{package}-{version} directory
    let _ = fs::remove_dir_all(
        HOME_ROOT_DIR
            .join(APPS_DIR)
            .join(format!("{}-{}", package, version)),
    );

    let _ = fs::remove_file(HOME_ROOT_DIR.join(APPS_DIR).join(package));

    // the generated erlang shipment is in the build/erlang-shipment directory
    // copy it to ～/.gleam_pkgs/apps/{package}-{version}
    let result = copy_dir_all(
        contents_dir.join("build").join("erlang-shipment"),
        HOME_ROOT_DIR
            .join(APPS_DIR)
            .join(format!("{}-{}", package, version)),
    );
    if let Err(e) = result {
        return Err(GleamPkgError::PackageBuildError(format!(
            "Failed to copy erlang shipment to apps directory: {}",
            e
        )));
    }

    // now let's create a shell named {package} in ~/.gleam_pkgs/apps
    // it should only do one thing: cd to the {package}-{version} directory
    // and run ./entrypoint.sh run
    // we should pass everything after the `./entrypoint.sh run`
    // as arguments to the entrypoint
    let shell = format!(
        "#!/bin/sh\ncd {}/{}/{}-{} && ./entrypoint.sh run \"$@\"\n",
        HOME_ROOT_DIR.display(),
        APPS_DIR,
        package,
        version
    );
    let shell_path = HOME_ROOT_DIR.join(APPS_DIR).join(package);
    fs::write(&shell_path, shell).map_err(|e| {
        GleamPkgError::PackageBuildError(format!(
            "Failed to create shell script in apps directory: {}",
            e
        ))
    })?;

    // enable the shell script to be executed
    let permissions = fs::metadata(&shell_path)?.permissions();
    let mut new_permissions = permissions.clone();
    new_permissions.set_mode(0o755);
    fs::set_permissions(&shell_path, new_permissions).map_err(|e| {
        GleamPkgError::PackageBuildError(format!(
            "Failed to set permissions on shell script: {}",
            e
        ))
    })?;

    path_check()?;

    println!(
        "Package installed successfully! You can run {} in your shell to use it now.",
        package
    );

    Ok(())
}

/// Recursively copy a directory and its contents to another directory
fn copy_dir_all(
    src: impl AsRef<std::path::Path>,
    dst: impl AsRef<std::path::Path>,
) -> Result<(), std::io::Error> {
    fs::create_dir_all(&dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}

/// check whether ~/.gleam_pkgs/apps is in the PATH
/// if not, show a prompt to ask user whether to add it to the shell profile
/// now support bash and zsh, other shells will need to add PATH manually
pub fn path_check() -> Result<(), GleamPkgError> {
    let user_shell = std::env::var("SHELL").unwrap();
    let user_shell = user_shell.split('/').last().unwrap();
    let user_shell = user_shell.split('.').next().unwrap();
    let user_shell = user_shell.to_lowercase();
    let profile = match user_shell.as_str() {
        "bash" => ".bashrc",
        "zsh" => ".zshrc",
        _ => {
            return Err(GleamPkgError::PathError(format!(
                "Unsupported shell: {}",
                user_shell
            )));
        }
    };
    let profile_path = dirs::home_dir().unwrap().join(profile);
    let current_path = std::env::var("PATH").unwrap();
    // println!("Current PATH: {}", current_path);
    let keywords = ".gleam_pkgs/apps";
    if current_path.contains(keywords) {
        return Ok(());
    }
    println!(
        "It seems that ~/.gleam_pkgs/apps is not in your PATH, \
do you want to add it to ~/{}? (y/n)",
        profile
    );
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();
    if input.trim() == "y" {
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(&profile_path)
            .map_err(|e| GleamPkgError::PathError(format!("Failed to open profile: {}", e)))?;
        file.write_all(b"\nexport PATH=$PATH:~/.gleam_pkgs/apps\n")
            .map_err(|e| GleamPkgError::PathError(format!("Failed to write to profile: {}", e)))?;
        println!(
            "PATH updated successfully please run `source ~/{}` to apply the changes",
            profile
        );
    }

    Ok(())
}
