pub trait IntoLeBytes {
    type Bytes;
    fn into_le_bytes(self) -> Self::Bytes;
}

pub trait FromLeBytes<T> {
    type Bytes;
    fn from_le_bytes(buf: Self::Bytes) -> T;
}

impl IntoLeBytes for u32 {
    type Bytes = [u8; 4];

    fn into_le_bytes(self) -> Self::Bytes {
        let b1: u8 = ((self >> 24) & 0xff) as u8;
        let b2: u8 = ((self >> 16) & 0xff) as u8;
        let b3: u8 = ((self >> 8) & 0xff) as u8;
        let b4: u8 = (self & 0xff) as u8;

        [b4, b3, b2, b1]
    }
}

impl FromLeBytes<u32> for u32 {
    type Bytes = [u8; 4];

    fn from_le_bytes(buf: Self::Bytes) -> u32 {
        ((buf[0] as u32) << 0)
            + ((buf[1] as u32) << 8)
            + ((buf[2] as u32) << 16)
            + ((buf[3] as u32) << 24)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hex;

    #[test]
    fn u32_to_bytes() {
        let x: u32 = 123456789;
        let bytes = x.into_le_bytes();
        let y = u32::from_le_bytes(bytes);

        assert_eq!(x, y);
        assert_eq!(hex::encode(bytes), "15cd5b07");
    }
}
