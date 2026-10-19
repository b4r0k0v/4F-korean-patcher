use std::{fs, path::Path};
pub fn cia(path: &Path, title: u64, entries: &[(u16, u32)]) {
    let n = 0x140 + 0xc4 + 64 * 0x24 + entries.len() * 0x30;
    let at = 0x2040usize;
    let content = (at + n).div_ceil(64) * 64;
    let mut data = vec![0; content + 16 * entries.len()];
    data[..4].copy_from_slice(&0x2020u32.to_le_bytes());
    data[16..20].copy_from_slice(&(n as u32).to_le_bytes());
    data[24..32].copy_from_slice(&(16 * entries.len() as u64).to_le_bytes());
    for (index, _) in entries {
        data[32 + *index as usize / 8] |= 0x80 >> (index % 8);
    }
    let t = &mut data[at..at + n];
    t[..4].copy_from_slice(&0x10004u32.to_be_bytes());
    t[0x140 + 0x4c..0x140 + 0x54].copy_from_slice(&title.to_be_bytes());
    t[0x140 + 0x9e..0x140 + 0xa0].copy_from_slice(&(entries.len() as u16).to_be_bytes());
    for (i, (index, id)) in entries.iter().enumerate() {
        let offset = 0x140 + 0xc4 + 64 * 0x24 + i * 0x30;
        t[offset..offset + 4].copy_from_slice(&id.to_be_bytes());
        t[offset + 4..offset + 6].copy_from_slice(&index.to_be_bytes());
    }
    fs::write(path, data).unwrap();
}
