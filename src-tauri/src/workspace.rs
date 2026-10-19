use crate::{
    error::{AppError, Result},
    model::{Cancel, Request},
};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub fn reject_link(path: &Path) -> Result<()> {
    let m = fs::symlink_metadata(path)?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if m.file_attributes() & 0x400 != 0 {
            return Err(AppError::new(
                "링크·접합점 경로는 작업에 사용할 수 없습니다.",
                path.display(),
            ));
        }
    }
    if m.file_type().is_symlink() {
        return Err(AppError::new(
            "링크 경로는 사용할 수 없습니다.",
            path.display(),
        ));
    }
    Ok(())
}
pub fn safe_tree(path: &Path) -> Result<()> {
    reject_link(path)?;
    if path.is_dir() {
        for item in fs::read_dir(path)? {
            safe_tree(&item?.path())?;
        }
    }
    Ok(())
}
pub fn copy(source: &Path, dest: &Path, cancel: &Cancel) -> Result<()> {
    reject_link(source)?;
    if dest.exists() {
        reject_link(dest)?;
    }
    let mut input = File::open(source)?;
    let mut output = File::create(dest)?;
    let mut buf = vec![0; 1024 * 1024];
    loop {
        cancel.check()?;
        let n = input.read(&mut buf)?;
        if n == 0 {
            break;
        }
        output.write_all(&buf[..n])?;
    }
    output.sync_all()?;
    Ok(())
}
pub fn equal(a: &Path, b: &Path, cancel: &Cancel) -> Result<()> {
    if fs::metadata(a)?.len() != fs::metadata(b)?.len() {
        return Err(AppError::new("출력 복사 크기가 다릅니다.", b.display()));
    }
    let mut a = File::open(a)?;
    let mut b = File::open(b)?;
    let mut x = vec![0; 1024 * 1024];
    let mut y = vec![0; x.len()];
    loop {
        cancel.check()?;
        let n = a.read(&mut x)?;
        if n == 0 {
            break;
        }
        b.read_exact(&mut y[..n])?;
        if x[..n] != y[..n] {
            return Err(AppError::new(
                "출력 복사 검증에 실패했습니다.",
                "byte comparison mismatch",
            ));
        }
    }
    Ok(())
}
pub struct Workspace {
    pub root: PathBuf,
    pub id: String,
    parent: PathBuf,
}
impl Workspace {
    pub fn create(r: &Request, id: &str) -> Result<Self> {
        let parent = fs::canonicalize(&r.work)?;
        let root = parent.join(format!("4f-run-{id}"));
        fs::create_dir(&root)?;
        let w = Self {
            root,
            id: id.into(),
            parent,
        };
        fs::write(w.root.join(".4f-owner"), id)?;
        for p in [
            &r.base,
            &r.patch1,
            &r.patch2,
            &r.output,
            &r.toolkit,
            &r.decryptor,
        ]
        .into_iter()
        .chain(r.dlc.iter())
        .chain(r.dlc_patch.iter())
        {
            if fs::canonicalize(p)?.starts_with(&w.root) {
                return Err(AppError::new(
                    "입력·출력은 실행 임시 폴더 밖에 있어야 합니다.",
                    p.display(),
                ));
            }
        }
        for p in ["base", "dlc", "patches", "logs", "results"] {
            fs::create_dir(w.root.join(p))?;
        }
        Ok(w)
    }
    pub fn cleanup(&self) -> Result<()> {
        let actual = fs::canonicalize(&self.root)?;
        if actual.parent() != Some(self.parent.as_path())
            || actual.file_name() != Some(std::ffi::OsStr::new(&format!("4f-run-{}", self.id)))
            || fs::read_to_string(actual.join(".4f-owner"))? != self.id
        {
            return Err(AppError::new(
                "소유 표시가 달라 임시 폴더를 보존합니다.",
                actual.display(),
            ));
        }
        safe_tree(&actual)?;
        fs::remove_dir_all(actual)?;
        Ok(())
    }
}
pub fn publish(source: &Path, output: &Path, name: &str, cancel: &Cancel) -> Result<PathBuf> {
    let dest = output.join(name);
    let part = output.join(format!(".{name}.partial"));
    // Reserve both names without overwriting pre-existing files.
    let reservation = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&dest)?;
    let result = (|| {
        let f = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&part)?;
        drop(f);
        copy(source, &part, cancel)?;
        equal(source, &part, cancel)?;
        cancel.check()?;
        drop(reservation);
        fs::rename(&part, &dest)?;
        Ok(dest.clone())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&dest); /* partial file is retained for diagnosis */
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ownership_marker_prevents_cleanup_of_foreign_data() {
        let t = tempfile::tempdir().unwrap();
        let parent = fs::canonicalize(t.path()).unwrap();
        let root = parent.join("4f-run-test");
        fs::create_dir(&root).unwrap();
        fs::write(root.join(".4f-owner"), "other").unwrap();
        fs::write(root.join("keep"), "existing").unwrap();
        let w = Workspace {
            root: root.clone(),
            parent,
            id: "test".into(),
        };
        assert!(w.cleanup().is_err());
        assert!(root.join("keep").exists());
        fs::write(root.join(".4f-owner"), "test").unwrap();
        w.cleanup().unwrap();
        assert!(t.path().is_dir());
        assert!(!root.exists());
    }
    #[test]
    fn output_collision_and_cancel_preserve_existing_files() {
        let t = tempfile::tempdir().unwrap();
        let input = t.path().join("input");
        fs::write(&input, "original").unwrap();
        fs::write(t.path().join("existing"), "keep").unwrap();
        assert!(publish(&input, t.path(), "existing", &Cancel::default()).is_err());
        assert_eq!(fs::read(t.path().join("existing")).unwrap(), b"keep");
        let c = Cancel::default();
        c.cancel();
        assert!(publish(&input, t.path(), "new", &c).is_err());
        assert!(!t.path().join("new").exists());
        assert_eq!(fs::read(input).unwrap(), b"original");
    }
    #[cfg(windows)]
    #[test]
    fn locked_input_is_rejected_and_cross_volume_copy_is_verified() {
        use std::os::windows::fs::OpenOptionsExt;
        let t = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();
        let source = t.path().join("original.3ds");
        fs::write(&source, "source data").unwrap();
        let lock = OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&source)
            .unwrap();
        assert!(crate::validation::input(&source, "3ds").is_err());
        assert!(copy(&source, &t.path().join("copy"), &Cancel::default()).is_err());
        drop(lock);
        crate::validation::input(&source, "3ds").unwrap();
        let output = tempfile::tempdir_in(std::env::temp_dir()).unwrap();
        let dest = publish(&source, output.path(), "copied", &Cancel::default()).unwrap();
        equal(&source, &dest, &Cancel::default()).unwrap();
    }
}
