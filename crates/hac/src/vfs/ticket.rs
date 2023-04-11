//! https://switchbrew.org/wiki/Ticket
//!
//! Allows easy access to ticket files, a format used to store an encrypted title keys.
//!
//! Cheap implementation only supporting 'common' Title key type.

use eyre::Result;
use fs_err as fs;
use std::{
    fmt,
    io::{self, Read, Seek},
    path::Path,
};
use tracing::{debug, info};

const COMMON_KEY_SIZE: u8 = 16;
// No. of hexadecimal characters
pub const SHORT_TITLEID_LEN: u8 = 16;

enum TicketData {
    TitleId = 0x2a0, // offset
    TitleKey = 0x180,
}

#[derive(Debug, Default, Clone)]
pub struct TitleKey {
    title_id: [u8; COMMON_KEY_SIZE as _],
    key: [u8; COMMON_KEY_SIZE as _],
}

impl fmt::Display for TitleKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}={}",
            hex::encode(self.title_id),
            hex::encode(self.key)
        )
    }
}

impl TitleKey {
    pub fn new<P: AsRef<Path>>(decrypted_tik_path: P) -> Result<TitleKey> {
        let mut title_key = TitleKey::default();
        let mut ticket = fs::File::open(decrypted_tik_path.as_ref())?;

        info!(tik = %decrypted_tik_path.as_ref().display(), "Reading ticket");

        ticket.seek(io::SeekFrom::Start(TicketData::TitleId as _))?;
        ticket.read_exact(&mut title_key.title_id)?;

        ticket.seek(io::SeekFrom::Start(TicketData::TitleKey as _))?;
        ticket.read_exact(&mut title_key.key)?;
        debug!(
            title_key = ?format!(
                "{}={}",
                hex::encode(title_key.title_id),
                hex::encode(title_key.key)
            )
        );

        Ok(title_key)
    }
}