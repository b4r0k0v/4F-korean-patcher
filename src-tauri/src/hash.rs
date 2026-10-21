use crate::{
    error::{AppError, Result},
    model::Cancel,
};
use sha2::{Digest, Sha256};
use std::{fs::File, io::Read, path::Path};

pub const BASE: &str = "5FDBC59C52BC2747488C21502E0BBCD67910A557640ADEEF878C19328EF20168";
pub const DLC: &str = "A18A1C190301F204602395B415BF37B2B4BDCD5C05709BBCD46EF7316936DB93";
pub fn digest(path: &Path, cancel: &Cancel) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hash = Sha256::new();
    let mut buf = vec![0; 1024 * 1024];
    loop {
        cancel.check()?;
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
    }
    Ok(format!("{:X}", hash.finalize()))
}
pub fn verify(path: &Path, expected: &str, cancel: &Cancel) -> Result<()> {
    let actual = digest(path, cancel)?;
    verify_digest(path, expected, &actual)
}
pub(crate) fn verify_digest(path: &Path, expected: &str, actual: &str) -> Result<()> {
    if actual != expected {
        return Err(AppError::new(
            "원본 SHA-256이 다릅니다. 지정된 버전의 원본을 선택하세요.",
            format!("{}\nexpected: {expected}\nactual: {actual}", path.display()),
        ));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn streaming_and_mismatch_and_cancel() {
        let f = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(f.path(), b"abc").unwrap();
        let c = Cancel::default();
        verify(
            f.path(),
            "BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD",
            &c,
        )
        .unwrap();
        std::fs::write(f.path(), b"changed").unwrap();
        assert!(verify(
            f.path(),
            "BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD",
            &c
        )
        .is_err());
        c.cancel();
        assert!(digest(f.path(), &c).is_err());
    }
}
