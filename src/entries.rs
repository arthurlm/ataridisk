use std::{mem, os::unix::prelude::MetadataExt, path::Path, slice, time::UNIX_EPOCH};

use chrono::prelude::*;

use crate::dos;

macro_rules! as_static_str {
    ($input:expr, $size:expr) => {{
        let mut result = [' ' as u8; $size];
        for (i, b) in $input.bytes().into_iter().enumerate() {
            assert!(i < result.len());
            result[i] = b;
        }
        result
    }};
}

fn format_datetime_to_atari(dt: NaiveDateTime) -> (u16, u16) {
    let time = (dt.second() / 2) as u16 | (dt.minute() << 5) as u16 | (dt.hour() << 11) as u16;
    let date = dt.day() as u16 | (dt.month() << 5) as u16 | ((dt.year() - 1980) << 9) as u16;

    (time, date)
}

/// Attribute that can be apply to storage entry.
#[derive(Debug)]
#[repr(u8)]
enum StorageAttr {
    None = 0x00,
    Directory = 0x10,
}

/// Storage entry as it is dump on disk
#[derive(Debug, Clone)]
#[repr(C)]
pub struct StorageEntry {
    /// Filestem
    name: [u8; 8],
    /// Extension
    ext: [u8; 3],
    /// Attributes
    attr: u8,
    /// Reserved (NT)
    _reserved1: u8,
    /// Creation time milliseconds
    ctime_ms: u8,
    /// Creation time
    ctime: u16,
    /// Creation date
    cdate: u16,
    /// Access ate
    adate: u16,
    /// Reserved (NT + OS2)
    _reserved2: u16,
    /// Last modification time
    mtime: u16,
    /// Last modification date
    mdate: u16,
    /// Start cluster index
    cluster_index: u16,
    /// File size
    size: u32,
}

impl StorageEntry {
    /// Create new entry
    fn new(
        name: [u8; 8],
        ext: [u8; 3],
        attr: u8,
        mtime_naive: NaiveDateTime,
        cluster_index: u16,
        size: u32,
    ) -> Self {
        let (mtime, mdate) = format_datetime_to_atari(mtime_naive);

        Self {
            name,
            ext,
            attr,
            _reserved1: 0,
            ctime_ms: 0,
            ctime: 0,
            cdate: 0,
            adate: 0,
            _reserved2: 0,
            mtime,
            mdate,
            cluster_index,
            size,
        }
    }

    // Create new entry from static information
    pub fn from_static_dir_info(filename: &str, extension: &str, cluster_index: u16) -> Self {
        let name = as_static_str!(filename, 8);
        let ext = as_static_str!(extension, 3);
        let attr = StorageAttr::Directory as u8;
        let mtime_naive = NaiveDateTime::new(
            NaiveDate::from_ymd(2021, 8, 1),
            NaiveTime::from_hms(12, 0, 0),
        );
        let size = 0;

        Self::new(name, ext, attr, mtime_naive, cluster_index, size)
    }

    /// Create a new entry from path
    pub fn from_path_and_index<P>(path: P, cluster_index: u16) -> Self
    where
        P: AsRef<Path>,
    {
        assert!(dos::is_valid_filename(&path));

        let path = path.as_ref();
        assert!(path.exists());

        let name = as_static_str!(path.file_stem().unwrap_or_default().to_str().unwrap(), 8);
        let ext = as_static_str!(path.extension().unwrap_or_default().to_str().unwrap(), 3);

        let attr = if path.is_dir() {
            StorageAttr::Directory
        } else {
            StorageAttr::None
        } as u8;

        let metadata = path.metadata().expect("Fail to get file metadata");

        let mtime = metadata
            .modified()
            .expect("Fail to get mtime")
            .duration_since(UNIX_EPOCH)
            .expect("Fail to get mtime timestamp")
            .as_secs();
        let mtime_naive = NaiveDateTime::from_timestamp(mtime as i64, 0);

        let size = metadata.size() as u32;

        Self::new(name, ext, attr, mtime_naive, cluster_index, size)
    }

    /// Create an emtpy entry.
    pub fn empty() -> Self {
        Self {
            name: [0; 8],
            ext: [0; 3],
            attr: 0,
            _reserved1: 0,
            ctime_ms: 0,
            ctime: 0,
            cdate: 0,
            adate: 0,
            _reserved2: 0,
            mtime: 0,
            mdate: 0,
            cluster_index: 0,
            size: 0,
        }
    }

    /// Return size of Self.
    ///
    /// This function is a wrapper above `std::mem`.
    #[inline]
    pub fn size_of() -> usize {
        mem::size_of::<Self>()
    }
}

/// List of all file contains on the disk.
#[derive(Debug)]
#[repr(C)]
pub struct StorageTable {
    entries: Vec<StorageEntry>,
    used_count: usize,
}

impl StorageTable {
    /// Create table with a given number of entries.
    pub fn new(count: usize) -> Self {
        assert!(count > 0);

        // Fill buffer with empty entries
        let entries = vec![StorageEntry::empty(); count];

        Self {
            entries,
            used_count: 0,
        }
    }

    /// Read table as a buffer of u8
    pub fn as_raw(&self) -> &[u8] {
        unsafe {
            slice::from_raw_parts(
                self.entries.as_ptr() as *const u8,
                self.entries.len() * StorageEntry::size_of(),
            )
        }
    }

    /// Add new entry to table and return true of false depending if
    /// table is full or not.
    pub fn push(&mut self, entry: StorageEntry) -> bool {
        if self.used_count >= self.entries.capacity() {
            log::warn!("Disk is out of inode. Cannot add new entry");
            return false;
        }

        self.entries[self.used_count] = entry;
        self.used_count += 1;

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXPECTED_ENTRY_SIZE: usize = 0x20;

    #[test]
    fn test_empty_rentry() {
        assert_eq!(StorageEntry::size_of(), EXPECTED_ENTRY_SIZE);

        // Check empty at init
        let mut table = StorageTable::new(3);
        assert_eq!(table.as_raw(), [0; EXPECTED_ENTRY_SIZE * 3]);

        // Check add success and fail the check emptyness
        assert!(table.push(StorageEntry::empty()));
        assert!(table.push(StorageEntry::empty()));
        assert!(table.push(StorageEntry::empty()));
        assert!(!table.push(StorageEntry::empty()));
        assert_eq!(table.as_raw(), [0; EXPECTED_ENTRY_SIZE * 3]);
    }

    #[test]
    fn test_content_entry() {
        let mut table = StorageTable::new(1);
        assert_eq!(table.as_raw(), [0; EXPECTED_ENTRY_SIZE * 1]);

        table.push(StorageEntry::from_path_and_index("./data/TEST.TXT", 0x1234));
        assert_eq!(
            table.as_raw(),
            [
                'T' as u8, 'E' as u8, 'S' as u8, 'T' as u8, ' ' as u8, ' ' as u8, ' ' as u8,
                ' ' as u8, // Filename
                'T' as u8, 'X' as u8, 'T' as u8, // Extension
                0x00,      // Attr
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // Padding
                0x74, 0x5A, // Time
                0x0B, 0x53, // Date
                0x34, 0x12, // Cluster index,
                0x30, 0x00, 0x00, 0x00, // Size
            ]
        );
    }

    #[test]
    fn test_from_static_dir_info() {
        let mut table = StorageTable::new(1);
        assert_eq!(table.as_raw(), [0; EXPECTED_ENTRY_SIZE * 1]);

        table.push(StorageEntry::from_static_dir_info("TEST", "TXT", 0x1234));
        assert_eq!(
            table.as_raw(),
            [
                'T' as u8, 'E' as u8, 'S' as u8, 'T' as u8, ' ' as u8, ' ' as u8, ' ' as u8,
                ' ' as u8, // Filename
                'T' as u8, 'X' as u8, 'T' as u8, // Extension
                0x10,      // Attr
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // Padding
                0x00, 0x60, // Time
                0x01, 0x53, // Date
                0x34, 0x12, // Cluster index,
                0x00, 0x00, 0x00, 0x00, // Size
            ]
        );
    }
}
