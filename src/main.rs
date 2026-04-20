use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;

use serde::Deserialize;
use winreg::enums::*;
use winreg::RegKey;

const INSTALL_DIR: &str = "C:\\Program Files\\backuptracker";
const JAR_NAME: &str = "backuptracker.jar";
const BAT_NAME: &str = "backuptracker.bat";

// GitHub API endpoint
const RELEASE_API: &str =
    "https://api.github.com/repos/mtchs-code-intern/backup-tracker/releases/latest";

fn main() {
    println!("Starting BackupTracker installer...");

    // 🔴 Enforce admin
    if !is_elevated() {
        println!("Requesting administrator privileges...");
        relaunch_as_admin();
        return;
    }

    if !is_java_installed() {
        eprintln!("Java is not installed or not in PATH.");
        eprintln!("Please install Java and re-run this installer.");
        return;
    }

    if let Err(e) = run_install() {
        eprintln!("Installation failed: {}", e);
        return;
    }

    println!("Installation completed successfully!");
}

fn run_install() -> io::Result<()> {
    create_install_dir()?;
    download_latest_jar()?;
    create_bat()?;
    add_to_path()?;
    Ok(())
}

fn is_java_installed() -> bool {
    Command::new("java")
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn create_install_dir() -> io::Result<()> {
    let path = Path::new(INSTALL_DIR);

    if !path.exists() {
        println!("Creating install directory...");
        fs::create_dir_all(path)?;
    } else {
        println!("Install directory already exists.");
    }

    Ok(())
}

//
// 🔴 Elevation logic
//
fn is_elevated() -> bool {
    Command::new("net")
        .arg("session")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn relaunch_as_admin() {
    let exe = std::env::current_exe().expect("Failed to get current exe");

    Command::new("powershell")
        .args([
            "-Command",
            &format!(
                "Start-Process -FilePath '{}' -Verb RunAs",
                exe.display()
            ),
        ])
        .spawn()
        .expect("Failed to relaunch as admin");
}

//
// 🔴 GitHub release structures
//
#[derive(Deserialize)]
struct Release {
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

//
// 🔴 Download latest JAR
//
fn download_latest_jar() -> io::Result<()> {
    println!("Downloading latest release from GitHub...");

    let client = reqwest::blocking::Client::new();

    let release: Release = client
        .get(RELEASE_API)
        .header("User-Agent", "backuptracker-installer")
        .send()
        .map_err(to_io_error)?
        .json()
        .map_err(to_io_error)?;

    let jar_asset = release
        .assets
        .iter()
        .find(|a| a.name.ends_with(".jar"))
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "No JAR asset found"))?;

    println!("Found asset: {}", jar_asset.name);

    let bytes = client
        .get(&jar_asset.browser_download_url)
        .send()
        .map_err(to_io_error)?
        .bytes()
        .map_err(to_io_error)?;

    let dest = Path::new(INSTALL_DIR).join(JAR_NAME);

    // ✅ FIX: borrow instead of move
    fs::write(&dest, &bytes)?;

    println!("Downloaded to {}", dest.display());

    Ok(())
}

fn to_io_error<E: std::fmt::Display>(e: E) -> io::Error {
    io::Error::new(io::ErrorKind::Other, e.to_string())
}

//
// 🔴 BAT launcher
//
fn create_bat() -> io::Result<()> {
    println!("Creating launcher...");

    let bat_path = Path::new(INSTALL_DIR).join(BAT_NAME);
    let mut file = File::create(bat_path)?;

    let contents = r#"@echo off
java -jar "C:\Program Files\backuptracker\backuptracker.jar" %*
"#;

    file.write_all(contents.as_bytes())?;
    Ok(())
}

//
// 🔴 PATH update
//
fn add_to_path() -> io::Result<()> {
    println!("Adding install directory to PATH...");

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let env = hkcu.open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)?;

    let current_path: String = env.get_value("PATH").unwrap_or_default();

    if !current_path.contains(INSTALL_DIR) {
        let new_path = if current_path.is_empty() {
            INSTALL_DIR.to_string()
        } else {
            format!("{};{}", current_path, INSTALL_DIR)
        };

        env.set_value("PATH", &new_path)?;
        println!("PATH updated. Restart terminal to apply.");
    } else {
        println!("PATH already contains install directory.");
    }

    Ok(())
}