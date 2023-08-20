// This file is used to download and extract prebuilt packages of maa-core.

use super::download::download_mirrors;
use super::extract::Archive;

use crate::dirs::{Dirs, Ensure};
use crate::maa_run::{command, SetLDLibPath};

use std::env::consts::{DLL_PREFIX, DLL_SUFFIX};
use std::env::var_os;
use std::path::{Component, Path, PathBuf};
use std::str::from_utf8;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use clap::ValueEnum;
use semver::Version;
use serde::Deserialize;
use tokio::runtime::Runtime;

pub struct MaaCore {
    channel: Channel,
}

pub const MAA_CORE_NAME: &str = if cfg!(target_os = "macos") {
    "libMaaCore.dylib"
} else if cfg!(target_os = "windows") {
    "MaaCore.dll"
} else {
    "libMaaCore.so"
};

fn extract_mapper(
    path: &Path,
    lib_dir: &Path,
    resource_dir: &Path,
    resource: bool,
) -> Option<PathBuf> {
    let mut components = path.components();
    while let Some(c) = components.next() {
        match c {
            Component::Normal(c) => {
                if resource && c == "resource" {
                    return Some(resource_dir.join(components.as_path()));
                } else if c
                    .to_str() // The DLL suffix may not the last part of the file name
                    .is_some_and(|s| s.starts_with(DLL_PREFIX) && s.contains(DLL_SUFFIX))
                {
                    return Some(lib_dir.join(c));
                } else {
                    continue;
                }
            }
            _ => continue,
        }
    }
    None
}

impl MaaCore {
    pub fn new(channel: Channel) -> Self {
        Self { channel }
    }

    pub fn version(&self, dirs: &Dirs) -> Result<Version> {
        let output = &command(dirs)?
            .set_ld_lib_path(dirs)
            .arg("version")
            .output()
            .context("Failed to run maa-run version")?
            .stdout;

        // Remove "MaaCore v" prefix and "\n" suffix
        let ver_str = from_utf8(&output[9..output.len() - 1]).context("Failed to parse output")?;
        Version::parse(ver_str).context("Failed to parse version")
    }

    pub fn install(&self, dirs: &Dirs, force: bool, no_resource: bool, t: u64) -> Result<()> {
        let lib_dir = &dirs.library().ensure()?;

        if lib_dir.join(MAA_CORE_NAME).exists() && !force {
            bail!("MaaCore already exists, use `maa update` to update it or `maa install --force` to force reinstall")
        }

        println!("Installing package (channel: {})...", self.channel);

        let cache_dir = &dirs.cache().ensure()?;
        let resource_dir = &dirs.resource().ensure_clean()?;

        let version_json = get_version_json(self.channel)?;
        let asset = &version_json.asset()?;
        let archive = asset.download(cache_dir, t)?;
        archive.extract(|path: &Path| extract_mapper(path, lib_dir, resource_dir, !no_resource))?;

        Ok(())
    }

    pub fn update(&self, dirs: &Dirs, no_resource: bool, t: u64) -> Result<()> {
        let version_json = get_version_json(self.channel)?;
        let current_version = self.version(dirs)?;
        let new_version = version_json.version();
        if current_version >= new_version {
            println!("MaaCore is already up to date: v{}.", current_version);
            return Ok(());
        }

        println!(
            "Found newer MaaCore version v{} current: v{}, updating...",
            new_version, current_version
        );

        let cache_dir = &dirs.cache().ensure()?;
        let asset = version_json.asset()?;
        let archive = asset.download(cache_dir, t)?;
        // Clean dirs before extracting, but not before downloading
        // because the download may be interrupted
        let lib_dir = &dirs.library().ensure_clean()?;
        let resource_dir = &dirs.resource().ensure_clean()?;
        archive.extract(|path: &Path| extract_mapper(path, lib_dir, resource_dir, !no_resource))?;

        Ok(())
    }
}

#[derive(ValueEnum, Clone, Copy, Default)]
pub enum Channel {
    #[default]
    Stable,
    Beta,
    Alpha,
}

impl From<&Channel> for &str {
    fn from(channel: &Channel) -> Self {
        match channel {
            Channel::Stable => "stable",
            Channel::Beta => "beta",
            Channel::Alpha => "alpha",
        }
    }
}

impl std::fmt::Display for Channel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s: &str = self.into();
        write!(f, "{}", s)
    }
}

fn get_version_json(channel: Channel) -> Result<VersionJSON> {
    let api_url = if let Some(url) = var_os("MAA_API_URL") {
        url.to_str().unwrap().to_owned()
    } else {
        "https://ota.maa.plus/MaaAssistantArknights/api/version".to_owned()
    };

    let url = format!("{}/{}.json", api_url, channel);
    let version_json: VersionJSON = reqwest::blocking::get(url)
        .context("Failed to get version json")?
        .json()
        .context("Failed to parse version json")?;
    Ok(version_json)
}

#[cfg_attr(test, derive(Debug, PartialEq))]
#[derive(Deserialize)]
pub struct VersionJSON {
    version: String,
    details: VersionDetails,
}

impl VersionJSON {
    pub fn version(&self) -> Version {
        Version::parse(&self.version[1..]).unwrap()
    }

    pub fn name(&self) -> Result<String> {
        let version = self.version();
        if cfg!(target_os = "macos") {
            Ok(format!("MAA-v{}-macos-runtime-universal.zip", version))
        } else if cfg!(target_os = "linux") {
            if cfg!(target_arch = "x86_64") {
                Ok(format!("MAA-v{}-linux-x86_64.tar.gz", version))
            } else if cfg!(target_arch = "aarch64") {
                Ok(format!("MAA-v{}-linux-aarch64.tar.gz", version))
            } else {
                Err(anyhow!(
                    "Unsupported architecture: {}",
                    std::env::consts::ARCH
                ))
            }
        } else if cfg!(target_os = "windows") {
            if cfg!(target_arch = "x86_64") {
                Ok(format!("MAA-v{}-win-x64.zip", version))
            } else if cfg!(target_arch = "aarch64") {
                Ok(format!("MAA-v{}-win-arm64.zip", version))
            } else {
                Err(anyhow!(
                    "Unsupported architecture: {}",
                    std::env::consts::ARCH
                ))
            }
        } else {
            Err(anyhow!("Unsupported platform"))
        }
    }

    pub fn asset(&self) -> Result<&Asset> {
        let asset_name = self.name()?;
        self.details
            .assets
            .iter()
            .find(|asset| asset.name == asset_name)
            .ok_or_else(|| anyhow!("Asset not found"))
    }
}

#[cfg_attr(test, derive(Debug, PartialEq))]
#[derive(Deserialize)]
pub struct VersionDetails {
    pub assets: Vec<Asset>,
}

#[cfg_attr(test, derive(Debug, PartialEq))]
#[derive(Deserialize)]
pub struct Asset {
    pub name: String,
    pub size: u64,
    pub browser_download_url: String,
    pub mirrors: Vec<String>,
}

impl Asset {
    pub fn download(&self, dir: &Path, t: u64) -> Result<Archive> {
        let path = dir.join(&self.name);
        let size = self.size;

        if path.exists() {
            let file_size = match path.metadata() {
                Ok(metadata) => metadata.len(),
                Err(_) => 0,
            };
            if file_size == size {
                println!("File {} already exists, skip download!", &self.name);
                return Archive::try_from(path);
            }
        }

        let url = &self.browser_download_url;
        let mirrors = self.mirrors.clone();

        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(t))
            .build()
            .context("Failed to build reqwest client")?;
        Runtime::new()
            .context("Failed to create tokio runtime")?
            .block_on(download_mirrors(&client, url, mirrors, &path, size, None))?;

        Archive::try_from(path)
    }
}