/// Give object capability to be network encoded of 4bytes.
///
/// This is usefull to encode number and size on network.
pub trait NetworkEncode {
    fn encode_network(&self) -> [u8; 4];
}

impl NetworkEncode for usize {
    fn encode_network(&self) -> [u8; 4] {
        (*self as u64).encode_network()
    }
}

impl NetworkEncode for u64 {
    fn encode_network(&self) -> [u8; 4] {
        let mut buf = [0; 4];
        buf[0] = (*self >> 24) as u8;
        buf[1] = (*self >> 16) as u8;
        buf[2] = (*self >> 8) as u8;
        buf[3] = *self as u8;
        buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode() {
        let expected = [0x12, 0x34, 0x56, 0x78];
        assert_eq!(0x12_34_56_78usize.encode_network(), expected);
        assert_eq!(0x12_34_56_78u64.encode_network(), expected);
    }
}
