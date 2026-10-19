use crate::{
    error::{AppError, Result},
    model::Cancel,
};
use std::{
    fs::{self, File},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::Duration,
};

pub trait Runner {
    fn run(&mut self, exe: &Path, cwd: &Path, args: &[String]) -> Result<()>;
    fn run_input(&mut self, exe: &Path, cwd: &Path, args: &[String], _input: &[u8]) -> Result<()> {
        self.run(exe, cwd, args)
    }
}
pub struct ProcessRunner {
    pub cancel: Cancel,
    pub logs: PathBuf,
    pub sequence: usize,
}
impl Runner for ProcessRunner {
    fn run(&mut self, exe: &Path, cwd: &Path, args: &[String]) -> Result<()> {
        self.execute(exe, cwd, args, None)
    }
    fn run_input(&mut self, exe: &Path, cwd: &Path, args: &[String], input: &[u8]) -> Result<()> {
        self.execute(exe, cwd, args, Some(input))
    }
}
impl ProcessRunner {
    fn execute(
        &mut self,
        exe: &Path,
        cwd: &Path,
        args: &[String],
        input: Option<&[u8]>,
    ) -> Result<()> {
        self.cancel.check()?;
        self.sequence += 1;
        let stem = self.logs.join(format!("{:03}", self.sequence));
        fs::write(
            stem.with_extension("command.txt"),
            format!(
                "exe: {}\ncwd: {}\nargs: {args:?}",
                exe.display(),
                cwd.display()
            ),
        )?;
        let stdout = stem.with_extension("stdout.log");
        let stderr = stem.with_extension("stderr.log");
        let mut command = Command::new(exe);
        command
            .current_dir(cwd)
            .args(args)
            .stdin(if input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(File::create(&stdout)?)
            .stderr(File::create(&stderr)?);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // Suspend until assigned to a Job, so descendants cannot escape cancellation.
            command.creation_flags(0x08000004);
        }
        let mut child = command.spawn()?;
        #[cfg(windows)]
        let job = match job::Job::attach(&child) {
            Ok(job) => job,
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(e);
            }
        };
        #[cfg(windows)]
        if let Err(e) = job.resume(child.id()) {
            let _ = job.terminate();
            let _ = child.kill();
            let _ = child.wait();
            return Err(e);
        }
        if let (Some(input), Some(mut stdin)) = (input, child.stdin.take()) {
            use std::io::Write;
            if let Err(e) = stdin.write_all(input) {
                #[cfg(windows)]
                job.terminate()?;
                let _ = child.kill();
                let _ = child.wait();
                return Err(e.into());
            }
        }
        loop {
            if let Err(e) = self.cancel.check() {
                #[cfg(windows)]
                job.terminate()?;
                let _ = child.kill();
                let status = child.wait()?;
                fs::write(
                    stem.with_extension("exit.txt"),
                    format!("cancelled: {status}"),
                )?;
                return Err(e);
            }
            if let Some(status) = child.try_wait()? {
                #[cfg(windows)]
                job.terminate()?;
                fs::write(stem.with_extension("exit.txt"), status.to_string())?;
                if !status.success() {
                    return Err(AppError::new(
                        "외부 도구가 실패했습니다. 도구 경로·원본과 상세 로그를 확인하세요.",
                        format!(
                            "{}\nexit: {status}\nstdout: {}\n{}\nstderr: {}\n{}",
                            exe.display(),
                            stdout.display(),
                            tail(&stdout),
                            stderr.display(),
                            tail(&stderr)
                        ),
                    ));
                }
                return Ok(());
            }
            thread::sleep(Duration::from_millis(80));
        }
    }
}
fn tail(p: &Path) -> String {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut f) = File::open(p) else {
        return String::new();
    };
    let size = f.metadata().map(|m| m.len()).unwrap_or(0);
    let _ = f.seek(SeekFrom::Start(size.saturating_sub(16_384)));
    let mut b = vec![];
    let _ = f.read_to_end(&mut b);
    String::from_utf8_lossy(&b).into_owned()
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    #[ignore = "subprocess fixture; invoked only by runner cancellation test"]
    fn process_fixture() {
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "tools::runner::tests::leaf_fixture", "--ignored"])
            .spawn()
            .unwrap();
        fs::write("fixture-child.pid", child.id().to_string()).unwrap();
        let _ = child.wait();
    }
    #[test]
    #[ignore = "subprocess fixture"]
    fn leaf_fixture() {
        thread::sleep(Duration::from_secs(30));
    }
    #[test]
    #[ignore = "stdin subprocess fixture"]
    fn stdin_fixture() {
        let mut line = String::new();
        assert!(std::io::stdin().read_line(&mut line).unwrap() > 0);
    }
    #[test]
    fn newline_input_releases_decryptor_prompt() {
        let t = tempfile::tempdir().unwrap();
        let mut runner = ProcessRunner {
            cancel: Cancel::default(),
            logs: t.path().into(),
            sequence: 0,
        };
        runner
            .run_input(
                &std::env::current_exe().unwrap(),
                t.path(),
                &[
                    "--exact".into(),
                    "tools::runner::tests::stdin_fixture".into(),
                    "--ignored".into(),
                ],
                b"\r\n",
            )
            .unwrap();
    }
    #[test]
    fn cancellation_terminates_descendants_and_keeps_logs() {
        let t = tempfile::tempdir().unwrap();
        let logs = t.path().join("logs");
        fs::create_dir(&logs).unwrap();
        let cancel = Cancel::default();
        let signal = cancel.clone();
        let pidfile = t.path().join("fixture-child.pid");
        let watcher = thread::spawn(move || {
            for _ in 0..100 {
                if pidfile.is_file() {
                    signal.cancel();
                    return;
                }
                thread::sleep(Duration::from_millis(50));
            }
            signal.cancel();
        });
        let mut runner = ProcessRunner {
            cancel,
            logs: logs.clone(),
            sequence: 0,
        };
        let error = runner
            .run(
                &std::env::current_exe().unwrap(),
                t.path(),
                &[
                    "--exact".into(),
                    "tools::runner::tests::process_fixture".into(),
                    "--ignored".into(),
                    "--nocapture".into(),
                ],
            )
            .unwrap_err();
        watcher.join().unwrap();
        assert_eq!(error.detail.as_ref(), "cancelled");
        let pid = fs::read_to_string(t.path().join("fixture-child.pid"))
            .unwrap()
            .parse::<u32>()
            .unwrap();
        unsafe {
            use windows_sys::Win32::{
                Foundation::{CloseHandle, WAIT_OBJECT_0},
                System::Threading::{OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE},
            };
            let process = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
            if !process.is_null() {
                assert_eq!(WaitForSingleObject(process, 1000), WAIT_OBJECT_0);
                CloseHandle(process);
            }
        }
        assert!(logs.join("001.command.txt").is_file());
    }
    #[test]
    fn nonzero_exit_and_missing_output_are_errors() {
        let t = tempfile::tempdir().unwrap();
        let mut runner = ProcessRunner {
            cancel: Cancel::default(),
            logs: t.path().into(),
            sequence: 0,
        };
        // Rust test harness returns a nonzero code for an unsupported argument.
        assert!(runner
            .run(
                &std::env::current_exe().unwrap(),
                t.path(),
                &["--invalid-test-option".into()]
            )
            .is_err());
        assert!(t.path().join("001.exit.txt").is_file());
        assert!(nonempty(&t.path().join("missing")).is_err());
    }
}
pub fn nonempty(p: &Path) -> Result<()> {
    if !p.is_file() || fs::metadata(p)?.len() == 0 {
        return Err(AppError::new(
            "도구가 예상 결과를 만들지 않았습니다. 상세 로그를 확인하세요.",
            p.display(),
        ));
    }
    Ok(())
}
#[cfg(windows)]
mod job {
    use super::*;
    use std::{
        mem::{size_of, zeroed},
        os::windows::io::AsRawHandle,
    };
    use windows_sys::Win32::{
        Foundation::{CloseHandle, HANDLE},
        System::JobObjects::*,
    };
    pub struct Job(HANDLE);
    impl Job {
        pub fn resume(&self, pid: u32) -> Result<()> {
            use windows_sys::Win32::{
                Foundation::INVALID_HANDLE_VALUE,
                System::{
                    Diagnostics::ToolHelp::*,
                    Threading::{OpenThread, ResumeThread, THREAD_SUSPEND_RESUME},
                },
            };
            unsafe {
                let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
                if snapshot == INVALID_HANDLE_VALUE {
                    return Err(std::io::Error::last_os_error().into());
                }
                let mut entry: THREADENTRY32 = zeroed();
                entry.dwSize = size_of::<THREADENTRY32>() as u32;
                let mut found = false;
                let mut ok = Thread32First(snapshot, &mut entry);
                while ok != 0 {
                    if entry.th32OwnerProcessID == pid {
                        let thread = OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID);
                        if !thread.is_null() {
                            found = ResumeThread(thread) != u32::MAX;
                            CloseHandle(thread);
                        }
                        break;
                    }
                    ok = Thread32Next(snapshot, &mut entry);
                }
                CloseHandle(snapshot);
                if !found {
                    return Err(AppError::new(
                        "외부 도구 스레드를 시작하지 못했습니다.",
                        pid,
                    ));
                }
                Ok(())
            }
        }
        pub fn attach(child: &std::process::Child) -> Result<Self> {
            unsafe {
                let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if handle.is_null() {
                    return Err(std::io::Error::last_os_error().into());
                }
                let job = Self(handle);
                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = zeroed();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                if SetInformationJobObject(
                    handle,
                    JobObjectExtendedLimitInformation,
                    &info as *const _ as *const _,
                    size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                ) == 0
                    || AssignProcessToJobObject(handle, child.as_raw_handle()) == 0
                {
                    return Err(std::io::Error::last_os_error().into());
                }
                Ok(job)
            }
        }
        pub fn terminate(&self) -> Result<()> {
            unsafe {
                if TerminateJobObject(self.0, 1) == 0 {
                    return Err(std::io::Error::last_os_error().into());
                }
            }
            loop {
                let mut info: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION = unsafe { zeroed() };
                let ok = unsafe {
                    QueryInformationJobObject(
                        self.0,
                        JobObjectBasicAccountingInformation,
                        &mut info as *mut _ as *mut _,
                        size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                        std::ptr::null_mut(),
                    )
                };
                if ok == 0 {
                    return Err(std::io::Error::last_os_error().into());
                }
                if info.ActiveProcesses == 0 {
                    return Ok(());
                }
                thread::sleep(Duration::from_millis(20));
            }
        }
    }
    impl Drop for Job {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}
