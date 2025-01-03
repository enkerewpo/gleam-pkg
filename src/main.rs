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

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
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
const _DB_FILE: &str = "db/metadata.json";

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

fn erl_eval(expr: &String) -> Result<String, GleamPkgError> {
    //  erl -noshell -eval 'expr' -s init stop
    let output = std::process::Command::new("erl")
        .arg("-noshell")
        .arg("-eval")
        .arg(expr)
        .arg("-s")
        .arg("init")
        .arg("stop")
        .output()
        .map_err(|e| {
            GleamPkgError::PackageBuildError(format!("Failed to run erl eval: {}, {}", expr, e))
        })?;
    if !output.status.success() {
        return Err(GleamPkgError::PackageBuildError(format!(
            "Failed to run erl eval: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
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

    // add gleescript to the package and run it
    // gleam add gleescript && gleam run -m gleescript -- --out=build
    let output = std::process::Command::new("gleam")
        .arg("add")
        .arg("gleescript")
        .current_dir(&contents_dir)
        .stderr(std::process::Stdio::inherit())
        .output()
        .map_err(|e| {
            GleamPkgError::PackageBuildError(format!(
                "Failed to run `gleam add gleescript` in contents directory: {}, {}",
                contents_dir.display(),
                e
            ))
        })?;
    if !output.status.success() {
        return Err(GleamPkgError::PackageBuildError(format!(
            "Failed to add gleescript: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    let output = std::process::Command::new("gleam")
        .arg("run")
        .arg("-m")
        .arg("gleescript")
        .arg("--")
        .arg("--out=build")
        .current_dir(&contents_dir)
        .stderr(std::process::Stdio::inherit())
        .output()
        .map_err(|e| {
            GleamPkgError::PackageBuildError(format!(
                "Failed to run `gleam run -m gleescript` in contents directory: {}, {}",
                contents_dir.display(),
                e
            ))
        })?;
    if !output.status.success() {
        return Err(GleamPkgError::PackageBuildError(format!(
            "Failed to run gleescript: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    // now we need to get current running erlang vm's version and other info
    // and embed it into the comment section of the escript

    let output = erl_eval(
        &"io:format(standard_io, \"~s~n\", [erlang:system_info(system_version)]).".to_string(),
    )?;
    let erlang_version = output.trim();
    println!("Erlang system version: {}", erlang_version);

    // let output = erl_eval(&format!(
    //     "io:format(\"~p~n\", [escript:extract(\"{}\", [])]).",
    //     contents_dir.join("build").join(package).display()
    // ))?;

    // println!(
    //     "Escript generated successfully: {} ...",
    //     &output[0..300],
    // );

    // first remove the existing ~/.gleam_pkgs/apps/{package}-{version} directory
    let _ = fs::remove_dir_all(
        HOME_ROOT_DIR
            .join(APPS_DIR)
            .join(format!("{}-{}", package, version)),
    );

    let _ = fs::remove_file(HOME_ROOT_DIR.join(APPS_DIR).join(package));

    // now we create another shell script to wrap the binary escript
    // this wrapper will detect os, exam erlang version compatibility
    // and eventually run the escript bundled inside the shell script
    let wrapper = HOME_ROOT_DIR.join(APPS_DIR).join(package);
    let mut file = fs::File::create(&wrapper).map_err(|e| {
        GleamPkgError::PackageBuildError(format!(
            "Failed to create wrapper script: {}, {}",
            wrapper.display(),
            e
        ))
    })?;

    // read binary escript's content in Vec<u8>
    let escript = fs::read(contents_dir.join("build").join(package)).map_err(|e| {
        GleamPkgError::PackageBuildError(format!(
            "Failed to read escript: {}, {}",
            contents_dir.join("build").join(package).display(),
            e
        ))
    })?;

    let escript_base64 = STANDARD.encode(&escript);

    let wrapper_code = format!(
        r#"#!/bin/sh
# This is a wrapper script for the escript generated by gleam-pkg

COMPILED_ERLANG_VERSION="{erlang_version}"

# Check if the Erlang version is the same
CURRENT_ERLANG_VERSION=$(erl -noshell -eval 'io:format("~s", [erlang:system_info(system_version)]).' -s init stop)

if [ "$CURRENT_ERLANG_VERSION" != "$COMPILED_ERLANG_VERSION" ]; then
    echo "Erlang version mismatch: compiled with $COMPILED_ERLANG_VERSION, running $CURRENT_ERLANG_VERSION"
    echo "Please recompile the package with the correct Erlang version"
    exit 1
fi

# Decode base64 content to a temporary file
TEMP_DIR=$(mktemp -d)
ESCRIPT_PATH="$TEMP_DIR/escript"
echo "{escript_base64}" | base64 -d > "$ESCRIPT_PATH"

# Make it executable
chmod +x "$ESCRIPT_PATH"

# Run the escript
"$ESCRIPT_PATH" "$@"

# Clean up
rm -rf "$TEMP_DIR"
"#
    );

    file.write_all(wrapper_code.as_bytes()).map_err(|e| {
        GleamPkgError::PackageBuildError(format!(
            "Failed to write to wrapper script: {}, {}",
            wrapper.display(),
            e
        ))
    })?;

    // add execute permission to the wrapper script using Unix permissions
    let mut perms = file.metadata().map_err(|e| {
        GleamPkgError::PackageBuildError(format!(
            "Failed to get metadata for wrapper script: {}, {}",
            wrapper.display(),
            e
        ))
    })?.permissions();
    perms.set_mode(0o755);
    file.set_permissions(perms).map_err(|e| {
        GleamPkgError::PackageBuildError(format!(
            "Failed to set permissions for wrapper script: {}, {}",
            wrapper.display(),
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
#[allow(dead_code)]
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
