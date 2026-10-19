use crate::{
    error::{AppError, Result},
    model::Format,
    tools::runner::{nonempty, Runner},
    validation,
};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub struct Toolkit {
    pub tool: PathBuf,
    pub ctr: PathBuf,
    pub makerom: PathBuf,
}
impl Toolkit {
    pub fn new(root: &Path) -> Result<Self> {
        Ok(Self {
            tool: validation::executable(root, &["3dstool.exe"])?,
            ctr: validation::ctr(root)?,
            makerom: validation::makerom(root)?,
        })
    }
    fn call(
        &self,
        runner: &mut dyn Runner,
        cwd: &Path,
        args: &[&str],
        expected: &[&str],
    ) -> Result<()> {
        runner.run(
            &self.tool,
            cwd,
            &args.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        )?;
        for p in expected {
            nonempty(&cwd.join(p))?;
        }
        Ok(())
    }
    pub fn extract_base(&self, runner: &mut dyn Runner, cwd: &Path) -> Result<()> {
        let mut args = vec![
            "-xtf".to_string(),
            "3ds".into(),
            "input.3ds".into(),
            "--header".into(),
            "HeaderNCCH.bin".into(),
        ];
        for i in 0..8 {
            args.extend([format!("-{i}"), format!("Partition{i}.bin")]);
        }
        runner.run(&self.tool, cwd, &args)?;
        nonempty(&cwd.join("HeaderNCCH.bin"))?;
        self.extract_primary(runner, cwd, "Partition0.bin")?;
        for (i, name) in [
            (1, "Manual"),
            (2, "DownloadPlay"),
            (6, "N3DSUpdate"),
            (7, "O3DSUpdate"),
        ] {
            if cwd.join(format!("Partition{i}.bin")).exists() {
                self.call(
                    runner,
                    cwd,
                    &[
                        "-xtf",
                        "cfa",
                        &format!("Partition{i}.bin"),
                        "--header",
                        &format!("HeaderNCCH{i}.bin"),
                        "--romfs",
                        &format!("Decrypted{name}.bin"),
                    ],
                    &[
                        &format!("HeaderNCCH{i}.bin"),
                        &format!("Decrypted{name}.bin"),
                    ],
                )?;
                self.call(
                    runner,
                    cwd,
                    &[
                        "-xtf",
                        "romfs",
                        &format!("Decrypted{name}.bin"),
                        "--romfs-dir",
                        &format!("Extracted{name}"),
                    ],
                    &[],
                )?;
            }
        }
        Ok(())
    }
    pub fn extract_primary(&self, runner: &mut dyn Runner, cwd: &Path, input: &str) -> Result<()> {
        self.call(
            runner,
            cwd,
            &[
                "-xtf",
                "cxi",
                input,
                "--header",
                "HeaderNCCH0.bin",
                "--exh",
                "DecryptedExHeader.bin",
                "--exefs",
                "DecryptedExeFS.bin",
                "--romfs",
                "DecryptedRomFS.bin",
                "--logo",
                "LogoLZ.bin",
                "--plain",
                "PlainRGN.bin",
            ],
            &[
                "HeaderNCCH0.bin",
                "DecryptedExHeader.bin",
                "DecryptedExeFS.bin",
                "DecryptedRomFS.bin",
            ],
        )?;
        self.call(
            runner,
            cwd,
            &[
                "-xtf",
                "exefs",
                "DecryptedExeFS.bin",
                "--exefs-dir",
                "ExtractedExeFS",
                "--header",
                "HeaderExeFS.bin",
            ],
            &["HeaderExeFS.bin", "ExtractedExeFS/code.bin"],
        )?;
        self.call(
            runner,
            cwd,
            &[
                "-xtf",
                "romfs",
                "DecryptedRomFS.bin",
                "--romfs-dir",
                "ExtractedRomFS",
            ],
            &[],
        )?;
        if !cwd.join("ExtractedRomFS").is_dir()
            || fs::read_dir(cwd.join("ExtractedRomFS"))?.next().is_none()
        {
            return Err(AppError::new(
                "RomFS 추출 폴더가 비어 있습니다.",
                cwd.display(),
            ));
        }
        for (from, to) in [("banner.bnr", "banner.bin"), ("icon.icn", "icon.bin")] {
            if cwd.join("ExtractedExeFS").join(from).is_file() {
                fs::rename(
                    cwd.join("ExtractedExeFS").join(from),
                    cwd.join("ExtractedExeFS").join(to),
                )?;
            }
        }
        Ok(())
    }
    fn primary(&self, runner: &mut dyn Runner, cwd: &Path) -> Result<()> {
        normalize_code_header(cwd)?;
        self.call(
            runner,
            cwd,
            &[
                "-ctf",
                "romfs",
                "CustomRomFS.bin",
                "--romfs-dir",
                "ExtractedRomFS",
            ],
            &["CustomRomFS.bin"],
        )?;
        for (from, to) in [("banner.bin", "banner.bnr"), ("icon.bin", "icon.icn")] {
            if cwd.join("ExtractedExeFS").join(from).is_file() {
                fs::rename(
                    cwd.join("ExtractedExeFS").join(from),
                    cwd.join("ExtractedExeFS").join(to),
                )?;
            }
        }
        self.call(
            runner,
            cwd,
            &[
                "-ctf",
                "exefs",
                "CustomExeFS.bin",
                "--exefs-dir",
                "ExtractedExeFS",
                "--header",
                "HeaderExeFS.bin",
            ],
            &["CustomExeFS.bin"],
        )?;
        let mut args = vec![
            "-ctf",
            "cxi",
            "CustomPartition0.bin",
            "--header",
            "HeaderNCCH0.bin",
            "--exh",
            "DecryptedExHeader.bin",
            "--exefs",
            "CustomExeFS.bin",
            "--romfs",
            "CustomRomFS.bin",
        ];
        for (flag, name) in [("--logo", "LogoLZ.bin"), ("--plain", "PlainRGN.bin")] {
            if cwd.join(name).is_file() && fs::metadata(cwd.join(name))?.len() > 0 {
                args.extend([flag, name]);
            }
        }
        self.call(runner, cwd, &args, &["CustomPartition0.bin"])
    }
    pub fn rebuild_base(
        &self,
        runner: &mut dyn Runner,
        cwd: &Path,
        format: Format,
    ) -> Result<PathBuf> {
        self.primary(runner, cwd)?;
        for (i, name) in [
            (1, "Manual"),
            (2, "DownloadPlay"),
            (6, "N3DSUpdate"),
            (7, "O3DSUpdate"),
        ] {
            if cwd.join(format!("HeaderNCCH{i}.bin")).is_file() {
                self.call(
                    runner,
                    cwd,
                    &[
                        "-ctf",
                        "romfs",
                        &format!("Custom{name}.bin"),
                        "--romfs-dir",
                        &format!("Extracted{name}"),
                    ],
                    &[&format!("Custom{name}.bin")],
                )?;
                self.rebuild_cfa(
                    runner,
                    cwd,
                    i,
                    &format!("Custom{name}.bin"),
                    &format!("CustomPartition{i}.bin"),
                )?;
            }
        }
        let output = format!("patched.{}", format.extension());
        match format {
            Format::ThreeDs => {
                let mut args = vec![
                    "-ctf".into(),
                    "3ds".into(),
                    output.clone(),
                    "--not-pad".into(),
                    "--header".into(),
                    "HeaderNCCH.bin".into(),
                ];
                for i in 0..8 {
                    let name = if [3, 4, 5].contains(&i) {
                        format!("Partition{i}.bin")
                    } else {
                        format!("CustomPartition{i}.bin")
                    };
                    if cwd.join(&name).exists() {
                        nonempty(&cwd.join(&name))?;
                        args.extend([format!("-{i}"), name]);
                    }
                }
                runner.run(&self.tool, cwd, &args)?;
            }
            Format::Cia => {
                let mut args = vec!["-f".into(), "cia".into(), "-o".into(), output.clone()];
                for i in 0..3 {
                    let name = format!("CustomPartition{i}.bin");
                    if cwd.join(&name).exists() {
                        args.extend(["-content".into(), format!("{name}:{i}:0x{i:02x}")]);
                    }
                }
                runner.run(&self.makerom, cwd, &args)?;
            }
        }
        let output = cwd.join(output);
        nonempty(&output)?;
        match format {
            Format::Cia => {
                let catalog = crate::tools::cia::read(&output)?;
                if !catalog.contents.contains_key(&0) {
                    return Err(AppError::new(
                        "본편 CIA에 주 콘텐츠가 없습니다.",
                        output.display(),
                    ));
                }
            }
            Format::ThreeDs => {
                use std::io::{Read, Seek, SeekFrom};
                let mut f = fs::File::open(&output)?;
                f.seek(SeekFrom::Start(0x100))?;
                let mut magic = [0; 4];
                f.read_exact(&mut magic)?;
                if &magic != b"NCSD" {
                    return Err(AppError::new(
                        "3DS 결과 헤더가 올바르지 않습니다.",
                        output.display(),
                    ));
                }
            }
        }
        Ok(output)
    }
    pub fn rebuild_cfa(
        &self,
        runner: &mut dyn Runner,
        cwd: &Path,
        index: usize,
        romfs: &str,
        output: &str,
    ) -> Result<()> {
        self.call(
            runner,
            cwd,
            &[
                "-ctf",
                "cfa",
                output,
                "--header",
                &format!("HeaderNCCH{index}.bin"),
                "--romfs",
                romfs,
            ],
            &[output],
        )
    }
}

fn normalize_code_header(cwd: &Path) -> Result<()> {
    let path = cwd.join("DecryptedExHeader.bin");
    let mut h = fs::read(&path)?;
    if h.len() < 0x40 {
        return Err(AppError::new(
            "ExHeader 크기가 올바르지 않습니다.",
            path.display(),
        ));
    }
    if h[0xd] & 1 != 0 {
        let pages = [0x14, 0x24, 0x34]
            .iter()
            .map(|at| u32::from_le_bytes(h[*at..*at + 4].try_into().unwrap()) as u64)
            .sum::<u64>();
        let code = fs::read(cwd.join("ExtractedExeFS/code.bin"))?;
        if pages > 0 && code.len() as u64 == pages * 4096 {
            // The provided UNDUB and Korean patch already contain raw, page-aligned code.
            h[0xd] &= !1;
            fs::write(path, h)?;
        } else {
            // Compressed code must have a plausible BLZ footer, never guess from the flag alone.
            let valid = code.len() >= 8 && {
                let at = code.len() - 8;
                let footer = u32::from_le_bytes(code[at..at + 4].try_into().unwrap());
                let expanded = u32::from_le_bytes(code[at + 4..].try_into().unwrap());
                let compressed = footer & 0xffffff;
                let header = footer >> 24;
                (8..=11).contains(&header)
                    && compressed >= header
                    && compressed as usize <= code.len()
                    && code.len() as u64 + expanded as u64 == pages * 4096
            };
            if !valid {
                return Err(AppError::new(
                    "code.bin 압축 형식과 ExHeader 크기가 일치하지 않습니다.",
                    cwd.display(),
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn raw_code_clears_only_compression_flag() {
        let t = tempfile::tempdir().unwrap();
        fs::create_dir(t.path().join("ExtractedExeFS")).unwrap();
        let mut h = vec![0; 2048];
        h[13] = 3;
        h[0x14..0x18].copy_from_slice(&1u32.to_le_bytes());
        fs::write(t.path().join("DecryptedExHeader.bin"), &h).unwrap();
        fs::write(t.path().join("ExtractedExeFS/code.bin"), vec![0; 4096]).unwrap();
        normalize_code_header(t.path()).unwrap();
        h[13] = 2;
        assert_eq!(fs::read(t.path().join("DecryptedExHeader.bin")).unwrap(), h);
    }
}
