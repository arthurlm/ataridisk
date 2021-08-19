use std::{io, time};

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

    #[error("invalid filename")]
    InvalidFilename,

    #[error("invalid chars")]
    InvalidChars,

    #[error("folder is full")]
    FolderFull,

    #[error("invalid time: {0}")]
    InvalidTime(#[from] time::SystemTimeError),

    #[error("invalid string: {0}")]
    StringParse(#[from] std::string::FromUtf8Error),

    #[error("invalid attributes")]
    InvalidAttr,
}

impl PartialEq for SerialDiskError {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (&*self, other),
            (Self::Serial(_), Self::Serial(_))
                | (Self::IO(_), &Self::IO(_))
                | (Self::DiskFull, Self::DiskFull)
                | (Self::InvalidFilename, Self::InvalidFilename)
                | (Self::InvalidChars, Self::InvalidChars)
                | (Self::FolderFull, Self::FolderFull)
                | (Self::InvalidTime(_), &Self::InvalidTime(_))
                | (Self::StringParse(_), &Self::StringParse(_))
                | (Self::InvalidAttr, Self::InvalidAttr)
        )
    }
}

pub type Result<T> = std::result::Result<T, SerialDiskError>;
