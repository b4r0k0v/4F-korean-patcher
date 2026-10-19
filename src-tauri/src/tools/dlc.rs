use crate::{
    error::{AppError, Result},
    model::Cancel,
    tools::{
        cia::{self, Cia},
        runner::{nonempty, Runner},
        toolkit::Toolkit,
    },
    validation, workspace,
};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub struct Decryptor {
    decrypt: PathBuf,
    makerom: PathBuf,
}
impl Decryptor {
    pub fn new(root: &Path) -> Result<Self> {
        Ok(Self {
            decrypt: validation::executable(root, &["decrypt.exe"])?,
            makerom: validation::executable(root, &["makerom.exe"])?,
        })
    }
    pub fn decrypt(&self, runner: &mut dyn Runner, cwd: &Path) -> Result<Cia> {
        let catalog = cia::read(&cwd.join("input.cia"))?;
        if catalog.title >> 32 != 0x0004008c {
            return Err(AppError::new(
                "DLC 타이틀의 CIA가 아닙니다.",
                format!("{:016X}", catalog.title),
            ));
        }
        // This PyInstaller executable writes beside its own executable, ignoring CWD.
        let local = cwd.join("decrypt.exe");
        workspace::copy(&self.decrypt, &local, &Cancel::default())?;
        runner.run_input(&local, cwd, &["input.cia".into()], b"\r\n")?;
        let mut args = vec![
            "-f".into(),
            "cia".into(),
            "-dlc".into(),
            "-ignoresign".into(),
            "-target".into(),
            "p".into(),
            "-o".into(),
            "decrypted.cia".into(),
        ];
        for (index, id) in &catalog.contents {
            let name = format!("input.{index}.ncch");
            nonempty(&cwd.join(&name))?;
            args.extend(["-i".into(), format!("{name}:{index}:{id}")]);
        }
        runner.run(&self.makerom, cwd, &args)?;
        nonempty(&cwd.join("decrypted.cia"))?;
        cia::same_contents(&catalog, &cwd.join("decrypted.cia"))?;
        Ok(catalog)
    }
    pub fn rebuild(
        &self,
        runner: &mut dyn Runner,
        toolkit: &Toolkit,
        cwd: &Path,
        catalog: &Cia,
        auxiliary: bool,
    ) -> Result<PathBuf> {
        for (i, name) in [(1, "DecryptedManual.bin"), (2, "DecryptedDownloadPlay.bin")] {
            if catalog.contents.contains_key(&(i as u16)) {
                toolkit.rebuild_cfa(runner, cwd, i, name, &format!("content.{i:04}.cfa"))?;
            }
        }
        let mut args = vec![
            "-f".into(),
            "cia".into(),
            "-dlc".into(),
            "-ignoresign".into(),
            "-target".into(),
            "p".into(),
            "-o".into(),
            "patched.cia".into(),
        ];
        if auxiliary {
            args.extend(["-rsf".into(), "dlc.rsf".into(), "-DSaveSize=0".into()]);
        }
        for (index, id) in &catalog.contents {
            let name = match index {
                0 if auxiliary => "content.0000.cfa".to_string(),
                1 | 2 => format!("content.{index:04}.cfa"),
                _ => format!("DecryptedApp.{index:04x}.{id:08x}"),
            };
            nonempty(&cwd.join(&name))?;
            args.extend(["-i".into(), format!("{name}:{index}:{id}")]);
        }
        runner.run(&self.makerom, cwd, &args)?;
        let out = cwd.join("patched.cia");
        nonempty(&out)?;
        cia::same_contents(catalog, &out)?;
        Ok(out)
    }
}
pub fn extract(
    runner: &mut dyn Runner,
    toolkit: &Toolkit,
    cwd: &Path,
    catalog: &Cia,
) -> Result<()> {
    runner.run(
        &toolkit.ctr,
        cwd,
        &["--contents=DecryptedApp".into(), "decrypted.cia".into()],
    )?;
    for (index, id) in &catalog.contents {
        nonempty(&cwd.join(format!("DecryptedApp.{index:04x}.{id:08x}")))?;
    }
    // DLC content 0 is data, not an application CXI. Keep all contents intact.
    for (index, name) in [(1, "Manual"), (2, "DownloadPlay")] {
        if let Some(id) = catalog.contents.get(&(index as u16)) {
            let header = format!("HeaderNCCH{index}.bin");
            let romfs = format!("Decrypted{name}.bin");
            runner.run(
                &toolkit.tool,
                cwd,
                &[
                    "-xtf".into(),
                    "cfa".into(),
                    format!("DecryptedApp.{index:04x}.{id:08x}"),
                    "--header".into(),
                    header.clone(),
                    "--romfs".into(),
                    romfs.clone(),
                ],
            )?;
            nonempty(&cwd.join(header))?;
            nonempty(&cwd.join(romfs))?;
        }
    }
    Ok(())
}
pub fn auxiliary(zip: &Path, cwd: &Path, cancel: &Cancel) -> Result<bool> {
    let mut z = zip::ZipArchive::new(fs::File::open(zip)?)?;
    let names = [
        ("rebuild/content.0000.cfa", "content.0000.cfa"),
        ("rebuild/dlc.rsf", "dlc.rsf"),
    ];
    let found = names
        .iter()
        .filter(|(n, _)| z.index_for_name(n).is_some())
        .count();
    if found == 0 {
        return Ok(false);
    }
    if found != 2 {
        return Err(AppError::new(
            "DLC 보조 빌드 데이터가 불완전합니다.",
            "rebuild/content.0000.cfa and rebuild/dlc.rsf must be paired",
        ));
    }
    for (name, output) in names {
        use std::io::{Read, Write};
        let mut input = z.by_name(name)?;
        let target = cwd.join(output);
        let mut f = fs::File::create(&target)?;
        let mut buf = [0; 65536];
        loop {
            cancel.check()?;
            let n = input.read(&mut buf)?;
            if n == 0 {
                break;
            }
            f.write_all(&buf[..n])?;
        }
        nonempty(&target)?;
        workspace::reject_link(&target)?;
    }
    Ok(true)
}
