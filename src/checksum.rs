use byteorder::{BigEndian, WriteBytesExt};

use crate::error;

/// Compute a CRC32 POSIX value for a given payload
/// to send, then write it to input writer.
pub fn write_crc32<W>(writer: &mut W, buf: &[u8]) -> error::Result<()>
where
    W: WriteBytesExt,
{
    // Compute hash
    let mut crc = crc_any::CRC::crc32posix();
    crc.digest(buf);
    let val = crc.get_crc();

    // Encode hash with correct endianess
    writer.write_u32::<BigEndian>(val as u32)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! assert_valid_crc32 {
        ($input:expr, $expected:expr) => {{
            let mut buf = vec![];
            write_crc32(&mut buf, $input).unwrap();
            assert_eq!(buf, $expected);
        }};
    }

    #[test]
    fn test_crc() {
        assert_valid_crc32!(&[0x00, 0x00, 0x00, 0x00, 0x00], [0xFF, 0xFF, 0xFF, 0xFF]);
        assert_valid_crc32!(&[0xFF, 0xFF, 0xFF, 0xFF, 0xFF], [0x09, 0x18, 0x04, 0xD7]);
        assert_valid_crc32!(&[0x01, 0x02, 0x03, 0x04, 0x05], [0x5A, 0x60, 0x0F, 0xE0]);
        assert_valid_crc32!(&[0x05, 0x04, 0x03, 0x02, 0x01], [0x4C, 0xA9, 0x21, 0xC5]);
    }
}
