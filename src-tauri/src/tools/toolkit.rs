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
                "-xutf",
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
