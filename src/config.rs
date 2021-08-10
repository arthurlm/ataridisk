use serde::Deserialize;

use crate::layout::{PartitionType, Tos};

#[derive(Debug, Default, Deserialize)]
pub struct Config {
    /// TOS version to use
    #[serde(default)]
    pub tos: Tos,

    /// Partition type to use
    #[serde(default)]
    pub partition_type: PartitionType,

    /// Number of sector to reserve for root directory
    #[serde(default)]
    root_directory_sectors: Option<u16>,
}

impl Config {
    /// Safe getter above root_directory_sectors
    pub fn root_directory_sectors(&self) -> u16 {
        self.root_directory_sectors.unwrap_or(8)
    }
}
