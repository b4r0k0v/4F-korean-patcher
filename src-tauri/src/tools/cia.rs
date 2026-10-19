//! CIA/TMD layout follows Project_CTR makerom/src/tmd.h and tmd.c.
use crate::error::{AppError, Result};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

#[derive(Debug, Clone)]
pub struct Cia {
    pub title: u64,
    pub contents: BTreeMap<u16, u32>,
}
pub fn read(path: &Path) -> Result<Cia> {
    let mut f = File::open(path)?;
    let size = f.metadata()?.len();
    let mut h = [0; 32];
    f.read_exact(&mut h)?;
    let le = |at| u32::from_le_bytes(h[at..at + 4].try_into().unwrap()) as u64;
    let align = |n: u64| n.div_ceil(64) * 64;
    if le(0) != 0x2020 {
        return Err(AppError::new(
            "CIA 헤더가 올바르지 않습니다.",
            path.display(),
        ));
    }
    let at = align(le(0)) + align(le(8)) + align(le(12));
    let n = le(16);
    if !(0x100..=16 * 1024 * 1024).contains(&n) || at + n > size {
        return Err(AppError::new(
            "CIA의 TMD 크기가 올바르지 않습니다.",
            path.display(),
        ));
    }
    f.seek(SeekFrom::Start(at))?;
    let mut t = vec![0; n as usize];
    f.read_exact(&mut t)?;
    parse_tmd(&t)
}
fn parse_tmd(t: &[u8]) -> Result<Cia> {
    let invalid = || {
        AppError::new(
            "CIA 콘텐츠 목록을 읽을 수 없습니다.",
            "invalid TMD signature/count/records",
        )
    };
    if t.len() < 4 {
        return Err(invalid());
    }
    let signature = u32::from_be_bytes(t[..4].try_into().unwrap());
    let body = match signature {
        0x10000 | 0x10003 => 0x240,
        0x10001 | 0x10004 => 0x140,
        0x10002 | 0x10005 => 0x7c,
        _ => return Err(invalid()),
    };
    if t.len() < body + 0xc4 {
        return Err(invalid());
    }
    let title = u64::from_be_bytes(t[body + 0x4c..body + 0x54].try_into().unwrap());
    let count = u16::from_be_bytes(t[body + 0x9e..body + 0xa0].try_into().unwrap()) as usize;
    let start = body + 0xc4 + 64 * 0x24;
    if count == 0 || start + count * 0x30 > t.len() {
        return Err(invalid());
    }
    let mut contents = BTreeMap::new();
    for record in t[start..start + count * 0x30].chunks_exact(0x30) {
        let id = u32::from_be_bytes(record[..4].try_into().unwrap());
        let index = u16::from_be_bytes(record[4..6].try_into().unwrap());
        if contents.insert(index, id).is_some() {
            return Err(invalid());
        }
    }
    Ok(Cia { title, contents })
}
pub fn same_contents(expected: &Cia, path: &Path) -> Result<()> {
    let actual = read(path)?;
    if actual.title != expected.title || actual.contents != expected.contents {
        return Err(AppError::new(
            "DLC 재생성 결과의 타이틀·콘텐츠 목록이 다릅니다.",
            format!("expected: {expected:?}\nactual: {actual:?}"),
        ));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_sparse_indices_and_ids() {
        let mut t = vec![0; 0x140 + 0xc4 + 64 * 0x24 + 3 * 0x30];
        t[..4].copy_from_slice(&0x10004u32.to_be_bytes());
        t[0x140 + 0x9e..0x140 + 0xa0].copy_from_slice(&3u16.to_be_bytes());
        for (i, index) in [0u16, 4, 32].iter().enumerate() {
            let at = 0x140 + 0xc4 + 64 * 0x24 + i * 0x30;
            t[at..at + 4].copy_from_slice(&(100 + i as u32).to_be_bytes());
            t[at + 4..at + 6].copy_from_slice(&index.to_be_bytes());
        }
        let c = parse_tmd(&t).unwrap();
        assert_eq!(c.contents.get(&32), Some(&102));
        assert!(!c.contents.contains_key(&1));
        t.truncate(t.len() - 1);
        assert!(parse_tmd(&t).is_err());
    }
}
