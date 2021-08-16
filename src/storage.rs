use std::{collections::HashMap, fmt::Debug, fs, io, mem, path::Path};

use crate::{
    entries::{StorageEntry, StorageTable},
    error::{self, SerialDiskError},
    fat::FileAllocationTable,
    layout::DiskLayout,
};

const ROOT_INDEX: u16 = 0;

#[derive(Debug)]
enum DiskBloc {
    Data(Vec<u8>),
    Entries(StorageTable),
}

#[derive(Debug)]
pub struct DiskStorage<'a> {
    /// Contains disk layout information and bytes mapping
    disk_layout: &'a DiskLayout,

    /// Content of the root sectors
    root_entries: Vec<StorageTable>,

    /// Content of the FAT sectors
    fat: FileAllocationTable,

    /// Bloc of data stored on disk
    sector_data: HashMap<u16, DiskBloc>,
}

impl<'a> DiskStorage<'a> {
    pub fn new(disk_layout: &'a DiskLayout) -> Self {
        // Init buffers
        let fat = FileAllocationTable::new(
            ((disk_layout.count_1fat_sectors() as usize * disk_layout.bytes_per_sector() as usize)
                / mem::size_of::<u16>())
                - disk_layout.first_free_cluster() as usize,
        );

        let root_entries = vec![
            StorageTable::new(
                disk_layout.bytes_per_sector() as usize / mem::size_of::<StorageEntry>(),
            );
            disk_layout.root_directory_sectors() as usize
        ];

        // Create struct
        Self {
            disk_layout,
            root_entries,
            fat,
            sector_data: HashMap::new(),
        }
    }

    pub fn read_sectors<W>(&self, writer: &mut W, index: u16, count: u16) -> io::Result<()>
    where
        W: io::Write,
    {
        for i in 0..count {
            self.read_sector(writer, index + i)?;
        }

        Ok(())
    }

    pub fn read_sector<W>(&self, writer: &mut W, index: u16) -> io::Result<()>
    where
        W: io::Write,
    {
        // Read buffer differently depending of sector location
        if index < self.disk_layout.count_fat_sectors() {
            log::debug!("Reading FAT: {:#04x}", index);
            self.read_fat_sector(writer, index)
        } else if index < self.disk_layout.first_free_sector() {
            log::debug!("Reading root sector: {:#04x}", index);
            self.read_root_sector(writer, index)
        } else {
            log::debug!("Reading data: {:#04x}", index);
            self.read_data_sector(writer, index)
        }
    }

    fn read_fat_sector<W>(&self, writer: &mut W, sector_index: u16) -> io::Result<()>
    where
        W: io::Write,
    {
        let bytes_per_sector = self.disk_layout.bytes_per_sector() as usize;
        let buf = self.fat.as_raw();

        let idx_start = (sector_index as usize - self.disk_layout.count_1fat_sectors() as usize)
            * bytes_per_sector;
        let idx_end = idx_start + bytes_per_sector;

        writer.write_all(&buf[idx_start..idx_end])
    }

    fn read_root_sector<W>(&self, writer: &mut W, sector_index: u16) -> io::Result<()>
    where
        W: io::Write,
    {
        let real_sector_index =
            sector_index as usize - self.disk_layout.count_fat_sectors() as usize;

        writer.write_all(self.root_entries[real_sector_index].as_raw())
    }

    fn read_data_sector<W>(&self, writer: &mut W, sector_index: u16) -> io::Result<()>
    where
        W: io::Write,
    {
        writer.write_all(match self.sector_data.get(&sector_index) {
            Some(DiskBloc::Data(data)) => data,
            Some(DiskBloc::Entries(entries)) => entries.as_raw(),
            None => panic!("Read uninitialized sector: {:#04x}", sector_index),
        })
    }

    pub fn import_path<P>(&mut self, path: P) -> error::Result<()>
    where
        P: AsRef<Path> + Debug,
    {
        self.import_sub_path(path, ROOT_INDEX)
    }

    pub fn import_sub_path<P>(&mut self, path: P, parent_index: u16) -> error::Result<()>
    where
        P: AsRef<Path> + Debug,
    {
        for (file_type, path) in fs::read_dir(path)?
            .into_iter()
            // Filter invalid read dir result
            .filter_map(|r| r.ok())
            // Skip hidden files
            .filter(|e| !e.file_name().to_str().unwrap().starts_with('.'))
            // Filter missing file type entry
            .filter_map(|e| {
                if let Ok(ft) = e.file_type() {
                    Some((ft, e.path()))
                } else {
                    None
                }
            })
        {
            if file_type.is_dir() {
                if let Err(e) = self.add_directory(&path, parent_index) {
                    log::warn!("Cannot add {:?} (error: {})", path, e);
                }
            } else if file_type.is_file() {
                if let Err(e) = self.add_file(&path, parent_index) {
                    log::warn!("Cannot add {:?} (error: {})", path, e);
                }
            } else {
                log::warn!("Skipping: {:?} (unhandled file type)", path);
            }
        }

        Ok(())
    }

    pub fn add_directory<P>(&mut self, path: P, parent_cluster_index: u16) -> error::Result<()>
    where
        P: AsRef<Path> + Debug,
    {
        log::debug!(
            "Adding directory: {:?} (parent {:#04x})",
            path,
            parent_cluster_index
        );

        // Create new entry in FAT
        let entry_cluster_index = self
            .fat
            .reserve_cluster()
            .ok_or(SerialDiskError::DiskFull)?;

        // Add entry for this folder
        self.add_storage_entry(
            StorageEntry::try_from_path_and_index(&path, entry_cluster_index)?,
            parent_cluster_index,
        )?;

        // Add . and .. in new folder
        self.add_storage_entry(
            StorageEntry::from_static_dir_info(".", "", entry_cluster_index),
            entry_cluster_index,
        )?;
        self.add_storage_entry(
            StorageEntry::from_static_dir_info("..", "", parent_cluster_index),
            entry_cluster_index,
        )?;

        // Import folder content
        self.import_sub_path(path, entry_cluster_index)?;

        Ok(())
    }

    pub fn add_file<P>(&mut self, path: P, parent_index: u16) -> error::Result<()>
    where
        P: AsRef<Path> + Debug,
    {
        log::debug!("Adding file: {:?} (parent: {:#04x})", path, parent_index);

        // Create some alias
        let bytes_per_sector = self.disk_layout.bytes_per_sector() as usize;
        let sectors_per_cluster = self.disk_layout.sectors_per_cluster() as usize;

        // Create first block for data
        let first_cluster_block_index = self
            .fat
            .reserve_cluster()
            .ok_or(SerialDiskError::DiskFull)?;

        let mut current_cluster_block_index = first_cluster_block_index;

        // Store content of the file in blocks
        let content = fs::read(&path)?;

        for (index, chunk) in content.chunks(bytes_per_sector).enumerate() {
            // Check if we have to extend block chain
            if index > 0 && index % sectors_per_cluster == 0 {
                current_cluster_block_index = self
                    .fat
                    .extend_cluster(current_cluster_block_index)
                    .ok_or(SerialDiskError::DiskFull)?;
            }

            // Compute sector index
            let current_sector_index = self
                .disk_layout
                .convert_cluster_to_sector(current_cluster_block_index)
                + (index % sectors_per_cluster) as u16;

            // Store data
            let mut chunk_stored = chunk.to_vec();
            chunk_stored.resize(bytes_per_sector, 0);
            self.sector_data
                .insert(current_sector_index, DiskBloc::Data(chunk_stored));
        }

        // Add to entry table
        self.add_storage_entry(
            StorageEntry::try_from_path_and_index(&path, first_cluster_block_index)?,
            parent_index,
        )?;

        Ok(())
    }

    fn add_storage_entry(&mut self, entry: StorageEntry, cluster_index: u16) -> error::Result<()> {
        if cluster_index == ROOT_INDEX {
            for i in 0..self.disk_layout.root_directory_sectors() as usize {
                if self.root_entries[i].push(entry.clone()).is_ok() {
                    return Ok(());
                }
            }

            Err(SerialDiskError::FolderFull)
        } else {
            self.add_storage_sub_entry(entry, cluster_index)
        }
    }

    fn add_storage_sub_entry(
        &mut self,
        entry: StorageEntry,
        cluster_index: u16,
    ) -> error::Result<()> {
        assert_ne!(cluster_index, ROOT_INDEX);

        let sector_index = self.disk_layout.convert_cluster_to_sector(cluster_index);

        // Try to add in the current sector
        if let Ok(()) = self.push_storage_bloc_entries(sector_index, entry.clone()) {
            return Ok(());
        }

        // Otherwise try the next sector
        if let Ok(()) = self.push_storage_bloc_entries(sector_index + 1, entry.clone()) {
            return Ok(());
        }

        // Still folder full ...
        // So we have no choice that getting a new cluster for this !
        let next_cluster = self
            .fat
            .extend_cluster(cluster_index)
            .ok_or(SerialDiskError::DiskFull)?;
        self.add_storage_sub_entry(entry, next_cluster)
    }

    fn push_storage_bloc_entries(
        &mut self,
        sector_index: u16,
        entry: StorageEntry,
    ) -> error::Result<()> {
        let table_size =
            self.disk_layout.bytes_per_sector() as usize / mem::size_of::<StorageEntry>();

        let bloc = self
            .sector_data
            .entry(sector_index)
            .or_insert_with(|| DiskBloc::Entries(StorageTable::new(table_size)));

        match bloc {
            DiskBloc::Entries(table) => table.push(entry),
            DiskBloc::Data(data) => {
                // Re-interpret data as StorageTable
                let mut table = StorageTable::try_from_reader(&mut data.as_slice(), table_size)?;
                table.push(entry)?;

                // Update stored bloc
                self.sector_data
                    .insert(sector_index, DiskBloc::Entries(table));

                Ok(())
            }
        }
    }
}
