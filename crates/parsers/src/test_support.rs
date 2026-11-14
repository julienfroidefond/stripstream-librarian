//! Test-only archive writers.

use std::path::Path;

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// Prefix a RAR 4 block (`HEAD_TYPE` onward) with its CRC16 (low half of the CRC32).
fn rar_block(body: &[u8]) -> Vec<u8> {
    let mut block = (crc32(body) as u16).to_le_bytes().to_vec();
    block.extend_from_slice(body);
    block
}

/// Write a minimal RAR 4 archive with `entries` stored uncompressed, in this order. There is
/// no RAR writer crate; the format of a stored archive is simple enough to emit by hand.
pub fn write_rar4(path: &Path, entries: &[(&str, &[u8])]) {
    let mut out = b"Rar!\x1a\x07\x00".to_vec();
    // Main archive header: type 0x73, no flags, size 13, two reserved fields.
    out.extend(rar_block(&[0x73, 0x00, 0x00, 13, 0x00, 0, 0, 0, 0, 0, 0]));
    for (name, data) in entries {
        let name = name.as_bytes();
        let mut body = vec![0x74];
        body.extend_from_slice(&0x8000u16.to_le_bytes()); // LONG_BLOCK: data follows
        body.extend_from_slice(&(32 + name.len() as u16).to_le_bytes());
        body.extend_from_slice(&(data.len() as u32).to_le_bytes()); // packed size
        body.extend_from_slice(&(data.len() as u32).to_le_bytes()); // unpacked size
        body.push(3); // host OS: Unix ('/' separators)
        body.extend_from_slice(&crc32(data).to_le_bytes());
        body.extend_from_slice(&0x5021_0000u32.to_le_bytes()); // DOS time
        body.push(20); // version needed
        body.push(0x30); // method: store
        body.extend_from_slice(&(name.len() as u16).to_le_bytes());
        body.extend_from_slice(&0o100644u32.to_le_bytes());
        body.extend_from_slice(name);
        out.extend(rar_block(&body));
        out.extend_from_slice(data);
    }
    // End of archive block.
    out.extend(rar_block(&[0x7B, 0x00, 0x40, 7, 0x00]));
    std::fs::write(path, out).expect("write rar");
}
