use std::mem::size_of;

use serde::Deserialize;

macro_rules! write_big_endian {
    ($buffer:expr, $idx:expr, $value:expr) => {{
        let value = $value;
        $buffer[$idx + 0] = ((value >> 8) & 0xff) as u8;
        $buffer[$idx + 1] = (value & 0xff) as u8;
    }};
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum PartitionType {
    Gem,
    Bgm,
}

impl PartitionType {
    /// Return the maximum sector size for this partition type.
    ///
    /// NB. This should be configurable for BGM but we prefer always
    /// use max size.
    pub fn bytes_per_sector(&self) -> u16 {
        match &*self {
            Self::Gem => 512,
            Self::Bgm => 8192,
        }
    }
}

impl Default for PartitionType {
    fn default() -> Self {
        Self::Bgm
    }
}

//. TOS supported versions.
#[derive(Debug, Clone, Deserialize)]
pub enum Tos {
    V100,
    V104,
}

impl Tos {
    #[inline]
    pub fn cluster_count(&self) -> u16 {
        match &*self {
            Self::V100 => 0x3FFF, // 14 bits
            Self::V104 => 0x7FFF, // 15 bits
        }
    }
}

impl Default for Tos {
    fn default() -> Self {
        Tos::V104
    }
}

/// Helper to represent FAT12 / FAT16 disk layout.
#[derive(Debug)]
pub struct DiskLayout {
    tos: Tos,
    partition_type: PartitionType,
    root_directory_sectors: u16,
}

impl DiskLayout {
    /// Create new disk layout.
    pub fn new(tos: Tos, partition_type: PartitionType, root_directory_sectors: u16) -> Self {
        Self {
            tos,
            partition_type,
            root_directory_sectors,
        }
    }

    /// Number of sectors for root directory.
    #[inline]
    pub fn root_directory_sectors(&self) -> u16 {
        self.root_directory_sectors
    }

    /// Number of sectors per cluster.
    #[inline]
    pub fn sectors_per_cluster(&self) -> u16 {
        2 // Always
    }

    /// Number of sector reserved at beginning of disk.
    #[inline]
    pub fn reserved_sector(&self) -> u16 {
        self.sectors_per_cluster() * 2
    }

    /// Bytes per sector.
    #[inline]
    pub fn bytes_per_sector(&self) -> u16 {
        self.partition_type.bytes_per_sector()
    }

    /// Bytes per cluster.
    #[inline]
    pub fn bytes_per_cluster(&self) -> u16 {
        self.bytes_per_sector() * self.sectors_per_cluster()
    }

    /// Bytes per disk
    #[inline]
    #[allow(dead_code)]
    pub fn bytes_per_disk(&self) -> u32 {
        self.bytes_per_cluster() as u32 * self.tos.cluster_count() as u32
    }

    #[inline]
    pub fn count_1fat_sectors(&self) -> u16 {
        self.tos.cluster_count() * size_of::<u16>() as u16 / self.bytes_per_sector() + 1
    }

    #[inline]
    pub fn count_2fat_sectors(&self) -> u16 {
        self.count_1fat_sectors()
    }

    #[inline]
    pub fn count_fat_sectors(&self) -> u16 {
        self.count_1fat_sectors() + self.count_2fat_sectors()
    }

    #[inline]
    pub fn first_free_sector(&self) -> u16 {
        self.count_fat_sectors() + self.root_directory_sectors
    }

    /// Convert disk layout to buffer that Atari can understand.
    pub fn bios_parameter_block(&self) -> [u8; 18] {
        let mut buffer = [0; 18];

        write_big_endian!(buffer, 0, self.bytes_per_sector());
        write_big_endian!(buffer, 2, self.sectors_per_cluster());
        write_big_endian!(buffer, 4, self.bytes_per_cluster());
        write_big_endian!(buffer, 6, self.root_directory_sectors());
        write_big_endian!(buffer, 8, self.count_1fat_sectors());
        write_big_endian!(buffer, 10, self.count_2fat_sectors());
        write_big_endian!(buffer, 12, self.first_free_sector());
        write_big_endian!(buffer, 14, self.tos.cluster_count());

        // Flags
        buffer[16] = 0; // 12Bit FAT
        buffer[17] = 1; // one FAT

        buffer
    }

    /// Convert cluster index to begin sector index.
    pub fn convert_cluster_to_sector(&self, cluster_index: u16) -> u16 {
        let sectors_per_cluster = self.sectors_per_cluster();
        let sector_offset = self.first_free_sector() - self.reserved_sector();

        sector_offset + cluster_index * sectors_per_cluster as u16
    }
}

impl Default for DiskLayout {
    fn default() -> Self {
        Self::new(Tos::V104, PartitionType::Bgm, 8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! layout {
        ($tos:expr, $pt:expr) => {
            layout!($tos, $pt, 8)
        };
        ($tos:expr, $pt:expr, $rdl:expr) => {
            DiskLayout::new($tos, $pt, $rdl)
        };
    }

    #[test]
    fn test_disk_size() {
        // GEM
        assert_eq!(
            layout!(Tos::V100, PartitionType::Gem).bytes_per_disk(),
            16_776_192
        );
        assert_eq!(
            layout!(Tos::V104, PartitionType::Gem).bytes_per_disk(),
            33_553_408
        );

        // BGM
        assert_eq!(
            layout!(Tos::V100, PartitionType::Bgm).bytes_per_disk(),
            268_419_072
        );
        assert_eq!(
            layout!(Tos::V104, PartitionType::Bgm).bytes_per_disk(),
            536_854_528
        );
    }

    #[test]
    fn test_const() {
        assert_eq!(PartitionType::Gem.bytes_per_sector(), 512);
        assert_eq!(PartitionType::Bgm.bytes_per_sector(), 8192);

        assert_eq!(Tos::V100.cluster_count(), ((1 << 14) - 1));
        assert_eq!(Tos::V104.cluster_count(), ((1 << 15) - 1));
    }

    #[test]
    fn test_bios_parameter_block() {
        let param = layout!(Tos::V100, PartitionType::Gem).bios_parameter_block();
        assert_eq!(
            param,
            [
                // Bytes per sector
                (512 >> 8) as u8,
                0x00,
                // Sector per cluster
                0x00,
                0x02,
                // Bytes per cluster
                ((512 * 2) >> 8) as u8,
                0x00,
                // Root directory length
                0x00,
                0x08,
                // Length of FAT in sector
                0x00,
                0x40,
                // Second FAT
                0x00,
                0x40,
                // First free sector
                0x00,
                0x40 + 0x40 + 0x08,
                // Total disk size
                0x3F,
                0xFF,
                // Flags
                0x00,
                0x01,
            ]
        );

        let param = layout!(Tos::V104, PartitionType::Bgm).bios_parameter_block();
        assert_eq!(
            param,
            [
                // Bytes per sector
                (8192 >> 8) as u8,
                0x00,
                // Sector per cluster
                0x00,
                0x02,
                // Bytes per cluster
                ((8192 * 2) >> 8) as u8,
                0x00,
                // Root directory length
                0x00,
                0x08,
                // Length of FAT in sector
                0x00,
                0x08,
                // Second FAT
                0x00,
                0x08,
                // First free sector
                0x00,
                0x08 + 0x08 + 0x08,
                // Total disk size
                0x7F,
                0xFF,
                // Flags
                0x00,
                0x01,
            ]
        );
    }

    #[test]
    fn test_convert_cluster_to_sector() {
        let layout = layout!(Tos::V104, PartitionType::Gem);
        assert_eq!(
            layout.convert_cluster_to_sector(0x00_50),
            0x00_50 * 2 + 0x01_04
        );
        assert_eq!(
            layout.convert_cluster_to_sector(0x01_00),
            0x01_00 * 2 + 0x01_04
        );

        let layout = layout!(Tos::V104, PartitionType::Bgm);
        assert_eq!(
            layout.convert_cluster_to_sector(0x00_50),
            0x00_50 * 2 + 0x00_14
        );
        assert_eq!(
            layout.convert_cluster_to_sector(0x01_00),
            0x01_00 * 2 + 0x00_14
        );
    }
}
