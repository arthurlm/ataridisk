use std::{mem, slice};

#[derive(Debug)]
#[repr(u16)]
enum ClusterValue {
    Free = 0x0000,
    Reserved = 0x0001,
    EndOfClusterChain = 0xFFFF,
}

#[derive(Debug)]
#[repr(C)]
pub struct FileAllocationTable {
    entries: Vec<u16>,
}

impl FileAllocationTable {
    pub fn new(count: usize) -> Self {
        assert!(count >= 2);

        let mut entries = vec![ClusterValue::Free as u16; count];

        // Mark first 2 entries as reserved
        entries[0] = ClusterValue::Reserved as u16;
        entries[1] = ClusterValue::Reserved as u16;

        Self { entries }
    }

    pub fn as_raw(&self) -> &[u8] {
        unsafe {
            slice::from_raw_parts(
                self.entries.as_ptr() as *const u8,
                self.entries.len() * mem::size_of::<u16>(),
            )
        }
    }

    /// Get new empty cluster
    pub fn reserve_cluster(&mut self) -> Option<u16> {
        self.entries
            .iter()
            .position(|x| *x == ClusterValue::Free as u16)
            .map(|next_index| {
                self.entries[next_index] = ClusterValue::EndOfClusterChain as u16;
                next_index as u16
            })
    }

    pub fn extend_cluster(&mut self, existing_index: u16) -> Option<u16> {
        // Check we extends an already existing cluster
        assert_eq!(
            self.entries[existing_index as usize],
            ClusterValue::EndOfClusterChain as u16,
            "Existing cluster index is not an ending index. Index {:#04x}",
            existing_index,
        );

        self.reserve_cluster().map(|next_index| {
            self.entries[existing_index as usize] = next_index;
            next_index
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reserve() {
        // Empty FAT
        let mut fat = FileAllocationTable::new(6);
        assert_eq!(
            fat.as_raw(),
            [
                0x01, 0x00, // Reserved
                0x01, 0x00, // Reserved
                0x00, 0x00, // 2
                0x00, 0x00, // 3
                0x00, 0x00, // 4
                0x00, 0x00, // 5
            ]
        );

        assert_eq!(fat.reserve_cluster(), Some(0x0002));
        assert_eq!(
            fat.as_raw(),
            [
                0x01, 0x00, // Reserved
                0x01, 0x00, // Reserved
                0xFF, 0xFF, // 2
                0x00, 0x00, // 3
                0x00, 0x00, // 4
                0x00, 0x00, // 5
            ]
        );

        assert_eq!(fat.reserve_cluster(), Some(0x0003));
        assert_eq!(fat.reserve_cluster(), Some(0x0004));
        assert_eq!(fat.reserve_cluster(), Some(0x0005));
        assert_eq!(fat.reserve_cluster(), None);
        assert_eq!(
            fat.as_raw(),
            [
                0x01, 0x00, // Reserved
                0x01, 0x00, // Reserved
                0xFF, 0xFF, // 2
                0xFF, 0xFF, // 3
                0xFF, 0xFF, // 4
                0xFF, 0xFF, // 5
            ]
        );
    }

    #[test]
    fn test_extend() {
        let mut fat = FileAllocationTable::new(7);
        assert_eq!(
            fat.as_raw(),
            [
                0x01, 0x00, // Reserved
                0x01, 0x00, // Reserved
                0x00, 0x00, // 2
                0x00, 0x00, // 3
                0x00, 0x00, // 4
                0x00, 0x00, // 5
                0x00, 0x00, // 6
            ]
        );

        let expected = [
            0x01, 0x00, // Reserved
            0x01, 0x00, // Reserved
            0x03, 0x00, // 2
            0x04, 0x00, // 3
            0xFF, 0xFF, // 4
            0x06, 0x00, // 5
            0xFF, 0xFF, // 6
        ];

        assert_eq!(fat.reserve_cluster(), Some(0x0002));
        assert_eq!(fat.extend_cluster(0x0002), Some(0x0003));
        assert_eq!(fat.extend_cluster(0x0003), Some(0x0004));
        assert_eq!(fat.reserve_cluster(), Some(0x0005));
        assert_eq!(fat.extend_cluster(0x0005), Some(0x0006));
        assert_eq!(fat.as_raw(), expected);

        assert_eq!(fat.extend_cluster(0x0004), None);
        assert_eq!(fat.extend_cluster(0x0006), None);
        assert_eq!(fat.as_raw(), expected);
    }

    #[test]
    #[should_panic(expected = "Existing cluster index is not an ending index.")]
    fn test_extend_panic() {
        let mut fat = FileAllocationTable::new(4);
        assert_eq!(fat.extend_cluster(0x0000), Some(0x0001));
    }
}
