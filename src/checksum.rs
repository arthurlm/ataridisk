use crate::network::NetworkEncode;

/// Compute a CRC32 POSIX value for a given payload
/// to send.
pub fn compute_buffer(buf: &[u8]) -> [u8; 4] {
    // Compute hash
    let mut crc = crc_any::CRC::crc32posix();
    crc.digest(buf);
    let val = crc.get_crc();

    // Encode hash with correct endianess
    val.encode_network()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crc() {
        assert_eq!(
            compute_buffer(&[0x00, 0x00, 0x00, 0x00, 0x00]),
            [0xFF, 0xFF, 0xFF, 0xFF]
        );
        assert_eq!(
            compute_buffer(&[0xFF, 0xFF, 0xFF, 0xFF, 0xFF]),
            [0x09, 0x18, 0x04, 0xD7]
        );
        assert_eq!(
            compute_buffer(&[0x01, 0x02, 0x03, 0x04, 0x05]),
            [0x5A, 0x60, 0x0F, 0xE0]
        );
        assert_eq!(
            compute_buffer(&[0x05, 0x04, 0x03, 0x02, 0x01]),
            [0x4C, 0xA9, 0x21, 0xC5]
        );
    }
}
