use std::io;

use thiserror::Error;

/// Common errors that can be use in the app.
#[derive(Debug, Error)]
pub enum SerialDiskError {
    #[error("serial: {0}")]
    Serial(#[from] serialport::Error),

    #[error("IO error: {0}")]
    IO(#[from] io::Error),

    #[error("Disk is full")]
    DiskFull,
}

pub type Result<T> = std::result::Result<T, SerialDiskError>;
