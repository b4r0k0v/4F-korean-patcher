use super::*;
use crate::{error::AppError, model::Format, test_support, tools::cia};
use std::{
    fs,
    io::Write,
    path::Path,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
use zip::{write::SimpleFileOptions, ZipWriter};

fn zip(path: &Path, entries: &[(&str, &str)]) {
    let mut z = ZipWriter::new(fs::File::create(path).unwrap());
    for (name, text) in entries {
        z.start_file(*name, SimpleFileOptions::default()).unwrap();
        z.write_all(text.as_bytes()).unwrap();
    }
    z.finish().unwrap();
}
fn fixture(root: &Path, format: Format, dlc: bool) -> Request {
    for folder in ["work", "output", "tools"] {
        fs::create_dir(root.join(folder)).unwrap();
    }
    for name in [
        "3dstool.exe",
        "CtrTool64.exe",
        "CtrTool32.exe",
        "MakeRom64.exe",
        "MakeRom32.exe",
        "decrypt.exe",
        "ctrtool.exe",
        "makerom.exe",
    ] {
        fs::write(root.join("tools").join(name), "fake").unwrap();
    }
    fs::write(root.join("base.3ds"), "original").unwrap();
    zip(
        &root.join("one.zip"),
        &[("ExeFS/code.bin", "one"), ("RomFS/text", "text")],
    );
    zip(
        &root.join("two.zip"),
        &[("patch/ExtractedExeFS/code.bin", "two")],
    );
    test_support::cia(
        &root.join("dlc.cia"),
        0x0004008c00123400,
        &(0..33).map(|i| (i, 100 + i as u32)).collect::<Vec<_>>(),
    );
    zip(
        &root.join("dlc.zip"),
        &[("patch/DecryptedApp.0020.00000084", "patched")],
    );
    Request {
        base: root.join("base.3ds"),
        patch1: root.join("one.zip"),
        patch2: root.join("two.zip"),
        dlc: dlc.then(|| root.join("dlc.cia")),
        dlc_patch: dlc.then(|| root.join("dlc.zip")),
        toolkit: root.join("tools"),
        decryptor: root.join("tools"),
        work: root.join("work"),
        output: root.join("output"),
        format,
    }
}
#[derive(Clone, Copy)]
enum Fault {
    None,
    Fail,
    Missing,
    Cancel,
}
struct Fake {
    cancel: Cancel,
    calls: Arc<AtomicUsize>,
    fault: Fault,
}
impl Runner for Fake {
    fn run(&mut self, exe: &Path, cwd: &Path, args: &[String]) -> Result<()> {
        self.cancel.check()?;
        let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        if call == 1 {
            match self.fault {
                Fault::Fail => return Err(AppError::new("도구 실패", "fake exit 1")),
                Fault::Missing => return Ok(()),
                Fault::Cancel => self.cancel.cancel(),
                Fault::None => {}
            }
        }
        let name = exe.file_name().unwrap().to_string_lossy().to_lowercase();
        if name == "decrypt.exe" {
            for index in cia::read(&cwd.join("input.cia"))?.contents.keys() {
                fs::write(cwd.join(format!("input.{index}.ncch")), "ncch")?;
            }
        } else if name.starts_with("ctrtool") {
            for (index, id) in cia::read(&cwd.join("decrypted.cia"))?.contents {
                fs::write(
                    cwd.join(format!("DecryptedApp.{index:04x}.{id:08x}")),
                    "content",
                )?;
            }
        } else if name.starts_with("makerom") {
            let output = &args[args.iter().position(|s| s == "-o").unwrap() + 1];
            let pairs = args
                .windows(2)
                .filter(|a| a[0] == "-i" || a[0] == "-content")
                .map(|a| {
                    let parts = a[1].split(':').collect::<Vec<_>>();
                    let id = parts[2]
                        .strip_prefix("0x")
                        .map(|s| u32::from_str_radix(s, 16).unwrap())
                        .unwrap_or_else(|| parts[2].parse().unwrap());
                    (parts[1].parse().unwrap(), id)
                })
                .collect::<Vec<_>>();
            test_support::cia(
                &cwd.join(output),
                if cwd.join("input.cia").exists() {
                    0x0004008c00123400
                } else {
                    0x0004000000123400
                },
                &pairs,
            );
        } else if args[0] == "-xtf" && args[1] == "3ds" {
            fs::write(cwd.join("HeaderNCCH.bin"), "header")?;
            fs::write(cwd.join("Partition0.bin"), "partition")?;
        } else if args[0].starts_with("-x") {
            for pair in args.windows(2) {
                if [
                    "--header", "--exh", "--exefs", "--romfs", "--logo", "--plain",
                ]
                .contains(&pair[0].as_str())
                {
                    fs::write(cwd.join(&pair[1]), vec![0; 2048])?;
                }
                if pair[0].ends_with("-dir") {
                    let dir = cwd.join(&pair[1]);
                    fs::create_dir_all(&dir)?;
                    fs::write(
                        dir.join(if pair[0] == "--exefs-dir" {
                            "code.bin"
                        } else {
                            "text"
                        }),
                        "extracted",
                    )?;
                }
            }
        } else if args[0] == "-ctf" {
            if args[1] == "exefs" {
                assert_eq!(fs::read(cwd.join("ExtractedExeFS/code.bin"))?, b"two");
            }
            let mut data = vec![0; 512];
            if args[1] == "3ds" {
                data[0x100..0x104].copy_from_slice(b"NCSD");
            }
            fs::write(cwd.join(&args[2]), data)?;
        } else {
            panic!("unexpected fake args: {args:?}");
        }
        Ok(())
    }
}
fn run_fake(r: Request, fault: Fault, emit: impl Fn(Progress)) -> (Result<Outcome>, usize) {
    let expected = hash::digest(&r.base, &Cancel::default()).unwrap();
    let dlc = hash::digest(&r.base.with_file_name("dlc.cia"), &Cancel::default()).unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let result = execute(
        r,
        Cancel::default(),
        emit,
        (&expected, &dlc),
        move |cancel, _| {
            Box::new(Fake {
                cancel,
                calls,
                fault,
            })
        },
    );
    (result, observed.load(Ordering::SeqCst))
}
#[test]
fn four_combinations_preserve_originals_and_cleanup() {
    for format in [Format::Cia, Format::ThreeDs] {
        for dlc in [false, true] {
            let t = tempfile::tempdir().unwrap();
            let r = fixture(t.path(), format, dlc);
            let original = fs::read(&r.base).unwrap();
            let (out, _) = run_fake(r.clone(), Fault::None, |_| {});
            let out = out.unwrap();
            assert_eq!(out.files.len(), if dlc { 2 } else { 1 });
            assert_eq!(fs::read(r.base).unwrap(), original);
            assert_eq!(fs::read_dir(r.work).unwrap().count(), 0);
            if dlc {
                assert_eq!(
                    cia::read(Path::new(&out.files[1])).unwrap().contents.len(),
                    33
                );
            }
        }
    }
}
#[test]
fn failure_missing_result_and_cancel_stop_following_stages() {
    for fault in [Fault::Fail, Fault::Missing, Fault::Cancel] {
        let t = tempfile::tempdir().unwrap();
        let r = fixture(t.path(), Format::Cia, false);
        let (out, calls) = run_fake(r.clone(), fault, |_| {});
        let e = out.unwrap_err();
        assert!(e.workspace.is_some());
        assert!(Path::new(e.workspace.as_ref().unwrap().as_ref()).is_dir());
        assert!(calls <= 2);
        assert_eq!(fs::read_dir(r.output).unwrap().count(), 0);
    }
}
#[test]
fn partial_publication_reports_saved_files_without_overwrite() {
    let t = tempfile::tempdir().unwrap();
    let r = fixture(t.path(), Format::Cia, true);
    let output = r.output.clone();
    let (out, _) = run_fake(r, Fault::None, |p| {
        if p.stage == "결과 저장·복사 검증" {
            fs::write(
                output.join(format!("SMT4F-KO-DLC-{}.cia", p.run_id)),
                "existing",
            )
            .unwrap();
        }
    });
    let e = out.unwrap_err();
    assert_eq!(e.saved_files.len(), 1);
    assert_eq!(
        fs::read(output.join(format!("SMT4F-KO-DLC-{}.cia", e.run_id.unwrap()))).unwrap(),
        b"existing"
    );
    assert!(Path::new(&e.saved_files[0]).is_file());
}
