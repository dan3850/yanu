use std::{
    ffi::OsStr,
    fmt,
    path::{Path, PathBuf},
    process::Command,
    str::FromStr,
};

use anyhow::{bail, Context, Result};
use strum_macros::EnumString;
use tracing::{debug, info};
use walkdir::WalkDir;

use crate::hac::backend::Backend;

use super::ticket::{self, TitleKey};

#[derive(Debug, Default, Clone)]
pub struct Nsp {
    pub path: PathBuf,
    pub title_key: Option<TitleKey>,
}

#[derive(Debug, Clone, EnumString)]
pub enum NcaType {
    Control,
    Program,
    Meta,
    Manual,
}

impl fmt::Display for NcaType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Clone)]
pub struct Nca {
    pub path: PathBuf,
    pub title_id: Option<String>,
    pub content_type: NcaType,
}

impl Nsp {
    pub fn from<P: AsRef<Path>>(path: P) -> Result<Self> {
        if path.as_ref().extension().context("no file found")? != "nsp" {
            bail!(
                "{:?} is not a nsp file",
                path.as_ref().file_name().context("no file found")?
            );
        }

        Ok(Self {
            path: path.as_ref().to_owned(),
            ..Default::default()
        })
    }
    pub fn extract_data_to<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let hactool = Backend::Hactool.path()?;

        info!("Extracting {:?}", &self.path);
        if !Command::new(hactool)
            .args([
                "-t",
                "pfs0",
                "--pfs0dir",
                &path.as_ref().to_string_lossy(),
                &self.path.to_string_lossy(),
            ])
            .status()?
            .success()
        {
            bail!("failed to extract {:?}", path.as_ref());
        }

        info!(
            "{:?} has been extracted in {:?}",
            self.path.file_name().context("no file found")?,
            path.as_ref()
        );

        Ok(())
    }
    pub fn derive_title_key<P: AsRef<Path>>(&mut self, data_path: P) -> Result<()> {
        if self.title_key.is_none() {
            info!("Deriving title key for {:?}", self.path.display());
            for entry in WalkDir::new(data_path.as_ref()) {
                let entry = entry?;
                match entry.path().extension().and_then(OsStr::to_str) {
                    Some("tik") => {
                        self.title_key = Some(ticket::get_title_key(&entry.path())?);
                        break;
                    }
                    _ => continue,
                }
            }
            if self.title_key.is_none() {
                bail!(
                    "Couldn't derive TitleKey, {:?} doesn't have a .tik file",
                    self.path
                );
            }
        } else {
            info!("TitleKey has already being derived!");
        }

        Ok(())
    }
    pub fn get_title_key(&self) -> String {
        match self.title_key {
            Some(ref key) => key.to_string(),
            None => "=".to_string(),
        }
    }
}

impl Nca {
    pub fn from<P: AsRef<Path>>(path: P) -> Result<Self> {
        if path.as_ref().extension().context("no file found")? != "nca" {
            bail!(
                "{:?} is not a nca file",
                path.as_ref().file_name().context("no file found")?
            );
        }

        info!(
            "Identifying title ID and content type for {:?}",
            path.as_ref()
        );

        let hactool = Backend::Hactool.path()?;

        let output = Command::new(&hactool).args([path.as_ref()]).output()?;
        if !output.status.success() {
            bail!("hactool failed to view info of {:?}", path.as_ref());
        }

        let stdout = std::str::from_utf8(output.stdout.as_slice())?.to_owned();
        let mut title_id: Option<String> = None;
        for line in stdout.lines() {
            if line.find("Title ID:").is_some() {
                title_id = Some(
                    line.trim()
                        .split(' ')
                        .last()
                        .expect("line must've had an item")
                        .into(),
                );
                debug!("Title ID: {:?}", title_id);
                break;
            }
        }

        let mut content_type: Option<NcaType> = None;
        for line in stdout.lines() {
            if line.find("Content Type:").is_some() {
                content_type = Some(
                    NcaType::from_str(
                        line.trim()
                            .split(' ')
                            .last()
                            .expect("line must've had an item"),
                    )
                    .context("failed to identify nca content type")?,
                );
                debug!("Content Type: {:?}", content_type);
                break;
            }
        }

        Ok(Self {
            path: path.as_ref().to_owned(),
            title_id,
            content_type: content_type.expect("content type should've been found"),
        })
    }
}
