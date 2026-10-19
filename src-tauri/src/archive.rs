use crate::{
    error::{AppError, Result},
    model::Cancel,
    workspace,
};
use std::{
    collections::HashSet,
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
};

#[derive(Clone, Copy)]
pub enum Layout {
    First,
    Patch,
}
fn relative(name: &str) -> Result<PathBuf> {
    let normalized = name.replace('\\', "/");
    let mut out = PathBuf::new();
    for c in normalized.trim_end_matches('/').split('/') {
        let stem = c.split('.').next().unwrap_or("").to_uppercase();
        if c.is_empty()
            || c == "."
            || c == ".."
            || c.contains([':', '<', '>', '"', '|', '?', '*'])
            || c.chars().any(|x| x.is_control())
            || c.ends_with([' ', '.'])
            || ["CON", "PRN", "AUX", "NUL"].contains(&stem.as_str())
            || (stem.len() == 4
                && (stem.starts_with("COM") || stem.starts_with("LPT"))
                && stem.ends_with(['1', '2', '3', '4', '5', '6', '7', '8', '9']))
        {
            return Err(AppError::new("ZIP에 안전하지 않은 경로가 있습니다.", name));
        }
        out.push(c);
    }
    Ok(out)
}
pub fn validate(path: &Path, layout: Layout, cancel: &Cancel) -> Result<()> {
    let mut zip = zip::ZipArchive::new(File::open(path)?)?;
    plan(&mut zip, layout, cancel).map(|_| ())
}
fn plan(
    zip: &mut zip::ZipArchive<File>,
    layout: Layout,
    cancel: &Cancel,
) -> Result<Vec<(usize, PathBuf, bool)>> {
    let mut entries = vec![];
    let mut seen = HashSet::new();
    let mut roots = HashSet::new();
    let mut targets = HashSet::new();
    let mut count = 0;
    // Inspect every entry, including entries outside patch/, before writing anything.
    for i in 0..zip.len() {
        cancel.check()?;
        let f = zip.by_index(i)?;
        let rel = relative(f.name())?;
        if f.unix_mode().is_some_and(|m| {
            m & 0o170000 != 0 && m & 0o170000 != 0o100000 && m & 0o170000 != 0o040000
        }) {
            return Err(AppError::new(
                "ZIP의 링크·특수 파일은 지원하지 않습니다.",
                f.name(),
            ));
        }
        if !seen.insert(rel.to_string_lossy().to_lowercase()) {
            return Err(AppError::new("ZIP에 중복 경로가 있습니다.", f.name()));
        }
        let mut components = rel.iter();
        let first = components.next().unwrap().to_string_lossy();
        roots.insert(first.to_string());
        let target = match layout {
            Layout::First => {
                let mut p = PathBuf::from(match first.as_ref() {
                    "ExeFS" => "ExtractedExeFS",
                    "RomFS" => "ExtractedRomFS",
                    x => x,
                });
                p.extend(components);
                p
            }
            Layout::Patch => {
                if first != "patch" {
                    continue;
                }
                components.collect::<PathBuf>()
            }
        };
        if target.as_os_str().is_empty() {
            continue;
        }
        if !targets.insert(target.to_string_lossy().to_lowercase()) {
            return Err(AppError::new(
                "폴더명 매핑 후 ZIP 경로가 중복됩니다.",
                f.name(),
            ));
        }
        if !f.is_dir() {
            count += 1;
        }
        entries.push((i, target, f.is_dir()));
    }
    if count == 0
        || (matches!(layout, Layout::Patch) && !roots.contains("patch"))
        || (matches!(layout, Layout::First)
            && (!roots.contains("ExeFS") || !roots.contains("RomFS")))
    {
        return Err(AppError::new(
            "패치 ZIP의 필수 폴더가 없습니다. 1차는 ExeFS/RomFS, 2차·DLC는 patch/가 필요합니다.",
            "invalid ZIP layout",
        ));
    }
    Ok(entries)
}
pub fn apply(zip: &Path, dest: &Path, layout: Layout, cancel: &Cancel) -> Result<()> {
    workspace::safe_tree(dest)?;
    let mut zip = zip::ZipArchive::new(File::open(zip)?)?;
    for (i, rel, dir) in plan(&mut zip, layout, cancel)? {
        cancel.check()?;
        let target = dest.join(rel);
        let parent = target.parent().unwrap();
        fs::create_dir_all(parent)?;
        // Existing tree was checked before extraction; newly created paths cannot be links.
        if dir {
            fs::create_dir_all(&target)?;
        } else {
            if target.exists() {
                workspace::reject_link(&target)?;
            }
            let mut input = zip.by_index(i)?;
            let mut out = File::create(&target)?;
            let mut buf = vec![0; 1024 * 1024];
            loop {
                cancel.check()?;
                let n = input.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                out.write_all(&buf[..n])?;
            }
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use zip::{write::SimpleFileOptions, ZipWriter};
    fn archive(path: &Path, files: &[(&str, &str)]) {
        let mut z = ZipWriter::new(File::create(path).unwrap());
        for (name, content) in files {
            z.start_file(*name, SimpleFileOptions::default()).unwrap();
            z.write_all(content.as_bytes()).unwrap();
        }
        z.finish().unwrap();
    }
    #[test]
    fn mapping_order_and_missing_patch() {
        let t = tempfile::tempdir().unwrap();
        let out = t.path().join("out");
        fs::create_dir(&out).unwrap();
        let z = t.path().join("p.zip");
        archive(&z, &[("ExeFS/code.bin", "one"), ("RomFS/text", "text")]);
        apply(&z, &out, Layout::First, &Cancel::default()).unwrap();
        archive(&z, &[("patch/ExtractedExeFS/code.bin", "two")]);
        apply(&z, &out, Layout::Patch, &Cancel::default()).unwrap();
        assert_eq!(
            fs::read(out.join("ExtractedExeFS/code.bin")).unwrap(),
            b"two"
        );
        archive(&z, &[("other/file", "no")]);
        assert!(apply(&z, &out, Layout::Patch, &Cancel::default()).is_err());
    }
    #[test]
    fn traversal_preflight_no_writes() {
        let t = tempfile::tempdir().unwrap();
        let z = t.path().join("p.zip");
        let out = t.path().join("out");
        fs::create_dir(&out).unwrap();
        archive(&z, &[("patch/good", "ok"), ("../evil", "bad")]);
        assert!(apply(&z, &out, Layout::Patch, &Cancel::default()).is_err());
        assert!(!out.join("good").exists());
        for bad in [
            "C:/x",
            "/absolute",
            "a/../../x",
            "a:ads",
            "a\\..\\x",
            "NUL.txt",
            "a. /x",
        ] {
            assert!(relative(bad).is_err(), "{bad}");
        }
    }
}
