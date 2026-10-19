use crate::{
    error::{AppError, Result},
    model::Request,
};
use std::{
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
};

pub fn executable(root: &Path, names: &[&str]) -> Result<PathBuf> {
    for name in names {
        let p = root.join(name);
        if p.is_file() {
            return Ok(fs::canonicalize(p)?);
        }
    }
    Err(AppError::new(
        "외부 도구 파일이 없습니다. ZIP을 해제한 폴더를 선택하세요.",
        format!("{}: {}", root.display(), names.join(", ")),
    ))
}
pub fn ctr(root: &Path) -> Result<PathBuf> {
    if cfg!(target_arch = "x86_64") {
        executable(root, &["CtrTool64.exe", "CtrTool32.exe", "ctrtool.exe"])
    } else {
        executable(root, &["CtrTool32.exe", "ctrtool.exe"])
    }
}
pub fn makerom(root: &Path) -> Result<PathBuf> {
    if cfg!(target_arch = "x86_64") {
        executable(root, &["MakeRom64.exe", "MakeRom32.exe", "makerom.exe"])
    } else {
        executable(root, &["MakeRom32.exe", "makerom.exe"])
    }
}
pub fn input(p: &Path, ext: &str) -> Result<()> {
    if !p.is_file() || !p.extension().is_some_and(|x| x.eq_ignore_ascii_case(ext)) {
        return Err(AppError::new(
            format!("읽을 수 있는 .{ext} 파일을 선택하세요."),
            p.display(),
        ));
    }
    File::open(p)?;
    Ok(())
}
pub fn writable(p: &Path) -> Result<()> {
    if !p.is_dir() {
        return Err(AppError::new(
            "작업·출력 폴더는 존재하는 폴더를 선택하세요.",
            p.display(),
        ));
    }
    let probe = p.join(format!(".4f-probe-{}", uuid::Uuid::new_v4()));
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)?;
    fs::remove_file(probe)?;
    Ok(())
}
pub fn validate(r: &Request) -> Result<()> {
    input(&r.base, "3ds")?;
    input(&r.patch1, "zip")?;
    input(&r.patch2, "zip")?;
    crate::archive::validate(
        &r.patch1,
        crate::archive::Layout::First,
        &crate::model::Cancel::default(),
    )?;
    crate::archive::validate(
        &r.patch2,
        crate::archive::Layout::Patch,
        &crate::model::Cancel::default(),
    )?;
    match (&r.dlc, &r.dlc_patch) {
        (Some(a), Some(b)) => {
            input(a, "cia")?;
            input(b, "zip")?;
            crate::archive::validate(
                b,
                crate::archive::Layout::Patch,
                &crate::model::Cancel::default(),
            )?;
        }
        (None, None) => {}
        _ => {
            return Err(AppError::new(
                "DLC 원본과 DLC 패치를 함께 선택하세요.",
                "incomplete DLC pair",
            ))
        }
    }
    executable(&r.toolkit, &["3dstool.exe"])?;
    ctr(&r.toolkit)?;
    makerom(&r.toolkit)?;
    executable(&r.decryptor, &["decrypt.exe"])?;
    executable(&r.decryptor, &["ctrtool.exe"])?;
    executable(&r.decryptor, &["makerom.exe"])?;
    writable(&r.work)?;
    writable(&r.output)?;
    // Legacy makerom uses ':' inside content arguments. Reject unsupported UNC paths early.
    let work = fs::canonicalize(&r.work)?;
    if work.to_string_lossy().starts_with(r"\\?\UNC\") {
        return Err(AppError::new(
            "작업 폴더는 로컬 드라이브를 선택하세요.",
            work.display(),
        ));
    }
    Ok(())
}
