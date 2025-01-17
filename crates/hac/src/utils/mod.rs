pub mod pack;
pub mod unpack;
pub mod update;

use crate::vfs::{nacp::NacpData, ticket::TitleKey};
use common::{
    defines::{DEFAULT_TITLEKEYS_PATH, SWITCH_DIR},
    error::MultiReport,
    utils::move_file,
};
use eyre::{bail, eyre, Result};
use fs_err as fs;
use std::{
    io::{self, ErrorKind},
    path::PathBuf,
};
use tracing::{info, warn};

pub fn clear_titlekeys() -> Result<()> {
    match fs::remove_file(DEFAULT_TITLEKEYS_PATH.as_path()) {
        Ok(_) => Ok(()),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
        Err(err) => {
            bail!(err)
        }
    }
}

/// Store TitleKeys to `DEFAULT_TITLEKEYS_PATH`.
pub fn store_titlekeys<'a, I>(keys: I) -> Result<()>
where
    I: Iterator<Item = &'a TitleKey>,
{
    info!(keyfile = ?SWITCH_DIR, "Storing TitleKeys");
    fs::create_dir_all(SWITCH_DIR.as_path())?;
    fs::write(
        DEFAULT_TITLEKEYS_PATH.as_path(),
        keys.map(|key| key.to_string())
            .collect::<Vec<_>>()
            .join("\n")
            + "\n",
    )
    .map_err(|err| eyre!(err))
}

#[derive(Debug, Default, Clone)]
pub struct CleanupDirsOnDrop {
    dirs: Vec<PathBuf>,
}

impl CleanupDirsOnDrop {
    pub fn new<I: IntoIterator<Item = PathBuf>>(dirs: I) -> Self {
        Self {
            dirs: dirs.into_iter().collect(),
        }
    }
    fn close_impl(&mut self) -> Result<()> {
        let errs = self
            .dirs
            .iter()
            .flat_map(|dir| fs::remove_dir_all(dir).err())
            .filter_map(|err| {
                if err.kind() != io::ErrorKind::NotFound {
                    Some(eyre!(err))
                } else {
                    None
                }
            })
            .inspect(|err| warn!(%err))
            .collect::<Vec<_>>();

        if errs.is_empty() {
            Ok(())
        } else {
            bail!(MultiReport::new(errs).join("\n"));
        }
    }
    pub fn close(mut self) -> Result<()> {
        let res = self.close_impl();
        std::mem::forget(self);
        res
    }
}

impl Drop for CleanupDirsOnDrop {
    fn drop(&mut self) {
        _ = self.close_impl();
    }
}

macro_rules! hacpack_cleanup_install {
    ($parent:expr) => {
        crate::utils::CleanupDirsOnDrop::new([
            $parent.join("hacpack_temp"),
            $parent.join("hacpack_backup"),
        ])
    };
}

pub(super) use hacpack_cleanup_install;

pub fn formatted_nsp_rename(
    nsp_path: &mut PathBuf,
    nacp_data: &NacpData,
    program_id: &str,
    suffix: &str,
) -> Result<()> {
    let dest = nsp_path
        .parent()
        .ok_or_else(|| eyre!("Failed to get parent"))?
        .join(format!(
            "{} [{}][v{}]{suffix}.nsp",
            nacp_data.get_application_name(),
            program_id,
            nacp_data.get_application_version()
        ));

    info!(from = %nsp_path.display(), to = %dest.display(), "Moving");
    move_file(&nsp_path, &dest)?;
    *nsp_path = dest;

    Ok(())
}
