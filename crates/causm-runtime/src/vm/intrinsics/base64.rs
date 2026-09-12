//! High-performance, pure-Rust Base64 encoder and decoder.
//!
//! Designed specifically for zero-allocation / minimal allocation
//! and direct interop with Causm memory payloads.

use causm_core::value::Payload;

const STANDARD_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

const DECODE_TABLE: [i8; 256] = {
    let mut table = [-1i8; 256];
    let mut i = 0usize;
    while i < 64 {
        table[STANDARD_ALPHABET[i] as usize] = i as i8;
        i += 1;
    }
    table
};

#[inline(always)]
pub fn encode_chunk(b0: u8, b1: u8, b2: u8) -> [u8; 4] {
    let i0 = (b0 >> 2) as usize;
    let i1 = (((b0 & 0x03) << 4) | (b1 >> 4)) as usize;
    let i2 = (((b1 & 0x0F) << 2) | (b2 >> 6)) as usize;
    let i3 = (b2 & 0x3F) as usize;
    [
        STANDARD_ALPHABET[i0],
        STANDARD_ALPHABET[i1],
        STANDARD_ALPHABET[i2],
        STANDARD_ALPHABET[i3],
    ]
}

#[inline(always)]
pub fn decode_chunk(c0: u8, c1: u8, c2: u8, c3: u8) -> Result<[u8; 3], String> {
    let v0 = DECODE_TABLE[c0 as usize];
    let v1 = DECODE_TABLE[c1 as usize];
    let v2 = if c2 == b'=' {
        0
    } else {
        DECODE_TABLE[c2 as usize]
    };
    let v3 = if c3 == b'=' {
        0
    } else {
        DECODE_TABLE[c3 as usize]
    };

    if v0 < 0 || v1 < 0 || (c2 != b'=' && v2 < 0) || (c3 != b'=' && v3 < 0) {
        return Err(format!(
            "Invalid base64 characters in chunk: [{}, {}, {}, {}]",
            c0 as char, c1 as char, c2 as char, c3 as char
        ));
    }

    let b0 = ((v0 as u8) << 2) | ((v1 as u8) >> 4);
    let b1 = (((v1 as u8) & 0x0F) << 4) | ((v2 as u8) >> 2);
    let b2 = (((v2 as u8) & 0x03) << 6) | (v3 as u8);

    Ok([b0, b1, b2])
}

/// Encodes a raw byte slice into a Base64 string.
pub fn encode_bytes(bytes: &[u8]) -> String {
    let n = bytes.len();
    if n == 0 {
        return String::new();
    }

    let out_len = n.div_ceil(3) * 4;
    let mut out = Vec::with_capacity(out_len);
    let mut i = 0;

    while i + 2 < n {
        let chunk = encode_chunk(bytes[i], bytes[i + 1], bytes[i + 2]);
        out.extend_from_slice(&chunk);
        i += 3;
    }

    if i + 1 == n {
        let b0 = bytes[i];
        let i0 = (b0 >> 2) as usize;
        let i1 = ((b0 & 0x03) << 4) as usize;
        out.push(STANDARD_ALPHABET[i0]);
        out.push(STANDARD_ALPHABET[i1]);
        out.push(b'=');
        out.push(b'=');
    } else if i + 2 == n {
        let b0 = bytes[i];
        let b1 = bytes[i + 1];
        let i0 = (b0 >> 2) as usize;
        let i1 = (((b0 & 0x03) << 4) | (b1 >> 4)) as usize;
        let i2 = ((b1 & 0x0F) << 2) as usize;
        out.push(STANDARD_ALPHABET[i0]);
        out.push(STANDARD_ALPHABET[i1]);
        out.push(STANDARD_ALPHABET[i2]);
        out.push(b'=');
    }

    // SAFETY: STANDARD_ALPHABET and '=' are ASCII / valid UTF-8.
    unsafe { String::from_utf8_unchecked(out) }
}

/// Encodes an array of Causm integer Payloads into a Base64 string.
pub fn encode_payload_array(arr: &[Payload]) -> String {
    let bytes: Vec<u8> = arr
        .iter()
        .map(|p| match p {
            Payload::Integer(i) => *i as u8,
            _ => 0,
        })
        .collect();
    encode_bytes(&bytes)
}

/// Decodes a Base64 string into a Vec of Causm integer Payloads.
pub fn decode_to_payload_array(b64: &str) -> Result<Vec<Payload>, String> {
    let bytes = b64.as_bytes();
    let n = bytes.len();
    if n == 0 {
        return Ok(Vec::new());
    }

    let mut out = Vec::with_capacity((n / 4) * 3);
    let mut i = 0;

    while i + 3 < n {
        let c0 = bytes[i];
        let c1 = bytes[i + 1];
        let c2 = bytes[i + 2];
        let c3 = bytes[i + 3];

        let chunk = decode_chunk(c0, c1, c2, c3)?;
        out.push(Payload::Integer(chunk[0] as i64));
        if c2 != b'=' {
            out.push(Payload::Integer(chunk[1] as i64));
        }
        if c3 != b'=' {
            out.push(Payload::Integer(chunk[2] as i64));
        }
        i += 4;
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_encode_chunk_and_roundtrip() {
        let chunk = encode_chunk(77, 97, 110);
        assert_eq!(&chunk, b"TWFu");

        let decoded = decode_chunk(84, 87, 70, 117).unwrap();
        assert_eq!(decoded, [77, 97, 110]);

        let encoded = encode_bytes(b"Hello Causm!");
        assert_eq!(encoded, "SGVsbG8gQ2F1c20h");

        let dec = decode_to_payload_array(&encoded).unwrap();
        let dec_bytes: Vec<u8> = dec
            .into_iter()
            .map(|p| match p {
                Payload::Integer(i) => i as u8,
                _ => 0,
            })
            .collect();
        assert_eq!(dec_bytes, b"Hello Causm!");
    }
}
