use crate::{
    defines::get_keyset_path,
    hac::{
        backend::Backend,
        rom::{Nca, NcaType},
    },
};

use super::rom::Nsp;
use anyhow::{bail, Context, Result};
use std::{env, ffi::OsStr, fs, path::PathBuf, process::Command};
use tempdir::TempDir;
use tracing::{info, warn};
use walkdir::WalkDir;

const TITLEID_SZ: u8 = 16;

pub fn patch_nsp_with_update(base: &mut Nsp, update: &mut Nsp) -> Result<Nsp> {
    let hactool = Backend::Hactool.path()?;
    let hacpack = Backend::Hacpack.path()?;

    let base_data_path = TempDir::new("basedata")?;
    let update_data_path = TempDir::new("updatedata")?;
    fs::create_dir_all(base_data_path.path())?;
    fs::create_dir_all(update_data_path.path())?;

    base.extract_data_to(base_data_path.path())?;
    update.extract_data_to(update_data_path.path())?;

    if let Err(err) = base.derive_title_key(base_data_path.path()) {
        warn!(
            "This error is not being handeled right away! {:?}",
            err.to_string()
        );
    }
    if let Err(err) = update.derive_title_key(update_data_path.path()) {
        warn!(
            "This error is not being handeled right away! {:?}",
            err.to_string()
        );
    }

    let switch_dir = dirs::home_dir()
        .context("failed to find home dir")?
        .join(".switch");
    fs::create_dir_all(&switch_dir)?;
    let title_keys_path = switch_dir.join("title.keys");

    info!("Saving TitleKeys in {:?}", title_keys_path.display());
    fs::write(
        &title_keys_path,
        format!("{}\n{}", base.get_title_key(), update.get_title_key()),
    )?;

    let mut base_nca: Option<Nca> = None;
    for entry in WalkDir::new(base_data_path.path())
        .sort_by(|a, b| {
            a.metadata()
                .expect(&format!("failed to read metadata for {:?}", a.path()))
                .len()
                .cmp(
                    &b.metadata()
                        .expect(&format!("failed to read metadata for {:?}", b.path()))
                        .len(),
                )
        })
        .into_iter()
        .filter_map(|e| e.ok())
    {
        match entry.path().extension().and_then(OsStr::to_str) {
            Some("nca") => {
                match Nca::from(entry.path()) {
                    Ok(nca) => {
                        match nca.content_type {
                            NcaType::Program => {
                                base_nca = Some(nca); // this will be the biggest NCA of 'Program' type
                                break;
                            }
                            _ => {}
                        };
                    }
                    Err(err) => {
                        warn!("{}", err.to_string());
                    }
                }
            }
            _ => {}
        }
    }
    let mut base_nca = base_nca.expect("base NCA must exist");

    let mut control_nca: Option<Nca> = None;
    let mut update_nca: Option<Nca> = None;
    for entry in WalkDir::new(update_data_path.path())
        .sort_by(|a, b| {
            a.metadata()
                .expect(&format!("failed to read metadata for {:?}", a.path()))
                .len()
                .cmp(
                    &b.metadata()
                        .expect(&format!("failed to read metadata for {:?}", b.path()))
                        .len(),
                )
        })
        .into_iter()
        .filter_map(|e| e.ok())
    {
        match entry.path().extension().and_then(OsStr::to_str) {
            Some("nca") => match Nca::from(entry.path()) {
                Ok(nca) => match nca.content_type {
                    NcaType::Control => {
                        if control_nca.is_none() {
                            control_nca = Some(nca);
                        }
                    }
                    NcaType::Program => {
                        if update_nca.is_none() {
                            update_nca = Some(nca);
                        }
                    }
                    _ => {}
                },
                Err(err) => {
                    warn!("{}", err.to_string());
                }
            },
            _ => {}
        }
    }
    let update_nca = update_nca.expect("update NCA must exist");
    let mut control_nca = control_nca.expect("control NCA must exist");

    let patch_dir = TempDir::new("patch")?;
    let romfs_dir = patch_dir.path().join("romfs");
    let exefs_dir = patch_dir.path().join("exefs");
    info!(
        "Extracting romfs/exefs from: {:?} {:?}",
        base_nca.path.display(),
        update_nca.path.display()
    );
    let status = Command::new(&hactool)
        .args([
            "--basenca",
            &base_nca.path.to_string_lossy(),
            &update_nca.path.to_string_lossy(),
            "--romfsdir",
            &romfs_dir.to_string_lossy(),
            "--exefsdir",
            &exefs_dir.to_string_lossy(),
        ])
        .status()?;
    if !status.success() {
        warn!(
            "The proccess responsible for extracting romfs/exefs terminated improperly {:?} (This might result in a crash!)",
            status.code()
        );
    }

    let nca_dir = patch_dir.path().join("nca");
    fs::create_dir_all(&nca_dir)?;
    fs::rename(
        &control_nca.path,
        &nca_dir.join(control_nca.path.file_name().expect("NCA file must exist")),
    )?;
    control_nca.path = nca_dir.join(control_nca.path.file_name().expect("NCA file must exist"));

    // Early cleanup
    info!("Cleaning up {:?}", base_data_path.path().display());
    drop(base_data_path);
    info!("Cleaning up {:?}", update_data_path.path().display());
    drop(update_data_path);

    let keyset_path = get_keyset_path()?;
    let mut title_id = base_nca.title_id.expect("base NCA must have title_id");
    title_id.truncate(TITLEID_SZ as _);
    info!("Packing romfs/exefs into a single NCA");
    if !Command::new(&hacpack)
        .args([
            "--keyset",
            &keyset_path.to_string_lossy(),
            "--type",
            "nca",
            "--ncatype",
            "program",
            "--plaintext",
            "--exefsdir",
            &exefs_dir.to_string_lossy(),
            "--romfsdir",
            &romfs_dir.to_string_lossy(),
            "--titleid",
            &title_id,
            "--outdir",
            &nca_dir.to_string_lossy(),
        ])
        .status()?
        .success()
    {
        bail!("failed to pack romfs/exefs into a single NCA");
    }

    let mut pactched_nca: Option<Nca> = None;
    for entry in WalkDir::new(&nca_dir).into_iter().filter_map(|e| e.ok()) {
        match entry.path().extension().and_then(OsStr::to_str) {
            Some("nca") => {
                pactched_nca = Some(Nca::from(entry.path())?);
                break;
            }
            _ => {}
        }
    }

    info!("Generating Meta NCA from patched NCA & control NCA");
    if !Command::new(&hacpack)
        .args([
            "--keyset",
            &keyset_path.to_string_lossy(),
            "--type",
            "nca",
            "--ncatype",
            "meta",
            "--titletype",
            "application",
            "--programnca",
            &pactched_nca
                .expect("patched NCA must exist")
                .path
                .to_string_lossy(),
            "--controlnca",
            &control_nca.path.to_string_lossy(),
            "--titleid",
            &title_id,
            "--outdir",
            &nca_dir.to_string_lossy(),
        ])
        .status()?
        .success()
    {
        bail!("failed to generate Meta NCA from patched NCA & control NCA");
    }

    // TODO: need to rewrite this aswell, prolly just take outdir as an arg in the fn
    let outdir: PathBuf;
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    {
        outdir = env::current_exe()?
            .parent()
            .expect("can't access parent dir of yanu")
            .to_owned();
    }
    #[cfg(target_os = "android")]
    {
        outdir = dirs::home_dir()
            .context("couldn't access home dir")?
            .join("storage")
            .join("shared");
    }
    let patched_nsp_path = outdir.join(format!("{}[yanu-patched].nsp", title_id));

    info!(
        "Packing all 3 NCAs into a NSP as {:?}",
        patched_nsp_path.display()
    );
    if !Command::new(&hacpack)
        .args([
            "--keyset",
            &keyset_path.to_string_lossy(),
            "--type",
            "nsp",
            "--ncadir",
            &nca_dir.to_string_lossy(),
            "--titleid",
            &title_id,
            "--outdir",
            &outdir.to_string_lossy(),
        ])
        .status()?
        .success()
    {
        bail!("Packing all 3 NCAs into a NSP");
    }

    Ok(Nsp::from(patched_nsp_path)?)
}
