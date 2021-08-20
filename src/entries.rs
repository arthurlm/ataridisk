use std::{io, mem, os::unix::prelude::MetadataExt, path::Path, slice, time::UNIX_EPOCH};

use byteorder::{NativeEndian, ReadBytesExt};
use chrono::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    dos,
    error::{self, SerialDiskError},
};

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

macro_rules! from_reader_static {
    ($reader:expr, $size:expr) => {{
        let mut result = [' ' as u8; $size];
        for i in 0..$size {
            result[i] = $reader.read_u8()?;
        }
        result
    }};
}

fn format_datetime_to_atari(dt: NaiveDateTime) -> (u16, u16) {
    let time = (dt.second() / 2) as u16 | (dt.minute() << 5) as u16 | (dt.hour() << 11) as u16;
    let date = dt.day() as u16 | (dt.month() << 5) as u16 | ((dt.year() - 1980) << 9) as u16;

    (time, date)
}

/// Attribute that can be apply to file.
#[derive(Debug)]
#[repr(u8)]
enum FileAttr {
    None = 0x00,
    Directory = 0x10,
}

/// Item as it is dump on disk
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[repr(C)]
pub struct FileInfo {
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
    pub cluster_index: u16,
    /// File size
    size: u32,
}

impl FileInfo {
    const EMPTY: Self = Self {
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
    };

    /// Create new file
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

    // Create new file from static information
    pub fn from_static_dir_info(filename: &str, extension: &str, cluster_index: u16) -> Self {
        let name = as_static_str!(filename, 8);
        let ext = as_static_str!(extension, 3);
        let attr = FileAttr::Directory as u8;
        let mtime_naive = NaiveDateTime::new(
            NaiveDate::from_ymd(2021, 8, 1),
            NaiveTime::from_hms(12, 0, 0),
        );
        let size = 0;

        Self::new(name, ext, attr, mtime_naive, cluster_index, size)
    }

    /// Create a new file from path
    pub fn try_from_path_and_index<P>(path: P, cluster_index: u16) -> error::Result<Self>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        assert!(path.exists());

        let (name, ext) = dos::as_valid_file_components(&path)?;

        let name = as_static_str!(name, 8);
        let ext = as_static_str!(ext, 3);

        let attr = if path.is_dir() {
            FileAttr::Directory
        } else {
            FileAttr::None
        } as u8;

        let metadata = path.metadata()?;
        let mtime = metadata.modified()?.duration_since(UNIX_EPOCH)?.as_secs();
        let mtime_naive = NaiveDateTime::from_timestamp(mtime as i64, 0);

        let size = metadata.size() as u32;

        Ok(Self::new(name, ext, attr, mtime_naive, cluster_index, size))
    }

    /// Create an file from any reader trait (vec, serial port, etc).
    pub fn try_from_reader<R>(reader: &mut R) -> io::Result<Self>
    where
        R: ReadBytesExt,
    {
        Ok(Self {
            name: from_reader_static!(reader, 8),
            ext: from_reader_static!(reader, 3),
            attr: reader.read_u8()?,
            _reserved1: reader.read_u8()?,
            ctime_ms: reader.read_u8()?,
            ctime: reader.read_u16::<NativeEndian>()?,
            cdate: reader.read_u16::<NativeEndian>()?,
            adate: reader.read_u16::<NativeEndian>()?,
            _reserved2: reader.read_u16::<NativeEndian>()?,
            mtime: reader.read_u16::<NativeEndian>()?,
            mdate: reader.read_u16::<NativeEndian>()?,
            cluster_index: reader.read_u16::<NativeEndian>()?,
            size: reader.read_u32::<NativeEndian>()?,
        })
    }

    pub fn filename(&self) -> error::Result<String> {
        let stem = String::from_utf8(self.name.to_vec())?;
        let ext = String::from_utf8(self.ext.to_vec())?;

        let stem = stem.trim();
        let ext = ext.trim();

        if ext.is_empty() {
            Ok(stem.to_string())
        } else {
            Ok(format!("{}.{}", stem, ext))
        }
    }

    pub fn is_dir(&self) -> bool {
        self.attr == FileAttr::Directory as u8
    }

    pub fn size(&self) -> usize {
        self.size as usize
    }
}

/// List of all file contains on the disk.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[repr(C)]
pub struct DirectoryContent {
    file_infos: Vec<FileInfo>,
}

impl DirectoryContent {
    /// Create table with a given number of entries.
    pub fn new(count: usize) -> Self {
        assert!(count > 0);

        Self {
            file_infos: vec![FileInfo::EMPTY; count],
        }
    }

    /// Create table from reader trait (ex: serial port)
    pub fn try_from_reader<R>(reader: &mut R, count: usize) -> io::Result<Self>
    where
        R: ReadBytesExt,
    {
        // Reserve some space
        let mut file_infos = Vec::with_capacity(count);

        // Read all the data from reader
        for _ in 0..count {
            let file_info = FileInfo::try_from_reader(reader)?;
            file_infos.push(file_info);
        }
        assert_eq!(file_infos.len(), count);

        Ok(Self { file_infos })
    }

    /// Read table as a buffer of u8
    pub fn as_raw(&self) -> &[u8] {
        unsafe {
            slice::from_raw_parts(
                self.file_infos.as_ptr() as *const u8,
                self.file_infos.len() * mem::size_of::<FileInfo>(),
            )
        }
    }

    /// Add new file to directory and return true of false depending if
    /// table is full or not.
    pub fn push(&mut self, file_info: FileInfo) -> error::Result<()> {
        self.file_infos
            .iter()
            .position(|e| *e == FileInfo::EMPTY)
            .map(|index| {
                self.file_infos[index] = file_info;
            })
            .ok_or(SerialDiskError::FolderFull)
    }

    pub fn as_vec(&self) -> Vec<FileInfo> {
        self.file_infos
            .iter()
            .filter(|e| **e != FileInfo::EMPTY)
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXPECTED_FILE_INFO_SIZE: usize = 0x20;

    #[test]
    fn test_size_of() {
        assert_eq!(mem::size_of::<FileInfo>(), EXPECTED_FILE_INFO_SIZE);
    }

    #[test]
    fn test_empty_init() {
        let table = DirectoryContent::new(3);
        assert_eq!(table.as_raw(), [0; EXPECTED_FILE_INFO_SIZE * 3]);
    }

    #[test]
    fn test_full() {
        let mut table = DirectoryContent::new(3);
        let file_info = FileInfo::try_from_path_and_index("./data/TEST.TXT", 0x1234).unwrap();

        // Check add success and fail the check emptyness
        assert_eq!(table.push(file_info.clone()), Ok(()));
        assert_eq!(table.push(file_info.clone()), Ok(()));
        assert_eq!(table.push(file_info.clone()), Ok(()));
        assert_eq!(
            table.push(file_info.clone()),
            Err(SerialDiskError::FolderFull)
        );
    }

    #[test]
    fn test_content() {
        let mut table = DirectoryContent::new(1);
        assert_eq!(table.as_raw(), [0; EXPECTED_FILE_INFO_SIZE * 1]);

        assert_eq!(
            table.push(FileInfo::try_from_path_and_index("./data/TEST.TXT", 0x1234).unwrap()),
            Ok(()),
        );
        assert_eq!(
            table.as_raw(),
            [
                'T' as u8, 'E' as u8, 'S' as u8, 'T' as u8, ' ' as u8, ' ' as u8, ' ' as u8,
                ' ' as u8, // Filename
                'T' as u8, 'X' as u8, 'T' as u8, // Extension
                0x00,      // Attr
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // Padding
                0xC0, 0x73, // Time
                0x01, 0x53, // Date
                0x34, 0x12, // Cluster index,
                0x14, 0x00, 0x00, 0x00, // Size
            ]
        );
    }

    #[test]
    fn test_from_static_dir_info() {
        let mut table = DirectoryContent::new(1);
        assert_eq!(table.as_raw(), [0; EXPECTED_FILE_INFO_SIZE * 1]);

        assert_eq!(
            table.push(FileInfo::from_static_dir_info("TEST", "TXT", 0x1234)),
            Ok(())
        );
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

    #[test]
    fn test_reader_fail() {
        let empty: Vec<u8> = vec![];
        assert!(FileInfo::try_from_reader(&mut empty.as_slice()).is_err());
    }

    #[test]
    fn test_reader_valid() {
        let data = vec![
            'T' as u8, 'E' as u8, 'S' as u8, 'T' as u8, ' ' as u8, ' ' as u8, ' ' as u8,
            ' ' as u8, // Filename
            'T' as u8, 'X' as u8, 'T' as u8, // Extension
            0x10,      // Attr
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // Padding
            0x00, 0x60, // Time
            0x01, 0x53, // Date
            0x34, 0x12, // Cluster index,
            0x00, 0x00, 0x00, 0x00, // Size
        ];

        let expected = FileInfo::from_static_dir_info("TEST", "TXT", 0x1234);

        assert_eq!(
            FileInfo::try_from_reader(&mut data.as_slice()).unwrap(),
            expected
        );
    }

    #[test]
    fn test_infos() {
        let file_info = FileInfo::from_static_dir_info("TEST", "TXT", 0x1234);

        assert!(file_info.is_dir());
        assert_eq!(file_info.filename().unwrap(), "TEST.TXT");
        assert_eq!(file_info.size(), 0);
    }

    #[test]
    fn test_list() {
        // Prepare a table with a lot of space in it
        let mut table = DirectoryContent::new(2096);
        let file_info = FileInfo::try_from_path_and_index("./data/TEST.TXT", 0x1234).unwrap();
        assert_eq!(table.push(file_info.clone()), Ok(()));
        assert_eq!(table.push(file_info.clone()), Ok(()));
        assert_eq!(table.push(file_info.clone()), Ok(()));

        // Check we have only added few files in list
        assert_eq!(table.as_vec(), vec![file_info; 3]);
    }
}
