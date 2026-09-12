//! Native Rust binary pack and unpack intrinsics.
//!
//! Provides fast, endian-aware byte packing and unpacking operations for Causm.

use causm_core::value::Payload;

#[inline(always)]
pub fn write_u16_be(val: i64) -> Vec<Payload> {
    let bytes = (val as u16).to_be_bytes();
    vec![
        Payload::Integer(bytes[0] as i64),
        Payload::Integer(bytes[1] as i64),
    ]
}

#[inline(always)]
pub fn write_u16_le(val: i64) -> Vec<Payload> {
    let bytes = (val as u16).to_le_bytes();
    vec![
        Payload::Integer(bytes[0] as i64),
        Payload::Integer(bytes[1] as i64),
    ]
}

#[inline(always)]
pub fn write_u32_be(val: i64) -> Vec<Payload> {
    let bytes = (val as u32).to_be_bytes();
    vec![
        Payload::Integer(bytes[0] as i64),
        Payload::Integer(bytes[1] as i64),
        Payload::Integer(bytes[2] as i64),
        Payload::Integer(bytes[3] as i64),
    ]
}

#[inline(always)]
pub fn write_u32_le(val: i64) -> Vec<Payload> {
    let bytes = (val as u32).to_le_bytes();
    vec![
        Payload::Integer(bytes[0] as i64),
        Payload::Integer(bytes[1] as i64),
        Payload::Integer(bytes[2] as i64),
        Payload::Integer(bytes[3] as i64),
    ]
}

#[inline(always)]
pub fn write_u64_be(val: i64) -> Vec<Payload> {
    let bytes = (val as u64).to_be_bytes();
    bytes.iter().map(|b| Payload::Integer(*b as i64)).collect()
}

#[inline(always)]
pub fn write_u64_le(val: i64) -> Vec<Payload> {
    let bytes = (val as u64).to_le_bytes();
    bytes.iter().map(|b| Payload::Integer(*b as i64)).collect()
}

#[inline(always)]
pub fn read_u16_be(arr: &[Payload]) -> i64 {
    let b0 = arr.first().and_then(payload_to_u8).unwrap_or(0);
    let b1 = arr.get(1).and_then(payload_to_u8).unwrap_or(0);
    u16::from_be_bytes([b0, b1]) as i64
}

#[inline(always)]
pub fn read_u16_le(arr: &[Payload]) -> i64 {
    let b0 = arr.first().and_then(payload_to_u8).unwrap_or(0);
    let b1 = arr.get(1).and_then(payload_to_u8).unwrap_or(0);
    u16::from_le_bytes([b0, b1]) as i64
}

#[inline(always)]
pub fn read_u32_be(arr: &[Payload]) -> i64 {
    let mut bytes = [0u8; 4];
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte = arr.get(i).and_then(payload_to_u8).unwrap_or(0);
    }
    u32::from_be_bytes(bytes) as i64
}

#[inline(always)]
pub fn read_u32_le(arr: &[Payload]) -> i64 {
    let mut bytes = [0u8; 4];
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte = arr.get(i).and_then(payload_to_u8).unwrap_or(0);
    }
    u32::from_le_bytes(bytes) as i64
}

#[inline(always)]
pub fn read_u64_be(arr: &[Payload]) -> i64 {
    let mut bytes = [0u8; 8];
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte = arr.get(i).and_then(payload_to_u8).unwrap_or(0);
    }
    u64::from_be_bytes(bytes) as i64
}

#[inline(always)]
pub fn read_u64_le(arr: &[Payload]) -> i64 {
    let mut bytes = [0u8; 8];
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte = arr.get(i).and_then(payload_to_u8).unwrap_or(0);
    }
    u64::from_le_bytes(bytes) as i64
}

#[inline(always)]
fn payload_to_u8(p: &Payload) -> Option<u8> {
    match p {
        Payload::Integer(i) => Some(*i as u8),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_binary_pack_unpack() {
        let p_be = write_u16_be(8080);
        assert_eq!(read_u16_be(&p_be), 8080);

        let p_le = write_u16_le(8080);
        assert_eq!(read_u16_le(&p_le), 8080);

        let u32_val = 305419896;
        let p32_be = write_u32_be(u32_val);
        assert_eq!(read_u32_be(&p32_be), u32_val);

        let p32_le = write_u32_le(u32_val);
        assert_eq!(read_u32_le(&p32_le), u32_val);
    }
}
