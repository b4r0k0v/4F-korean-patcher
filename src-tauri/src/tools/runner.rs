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
}
pub struct ProcessRunner {
    pub cancel: Cancel,
    pub logs: PathBuf,
    pub sequence: usize,
}
impl Runner for ProcessRunner {
    fn run(&mut self, exe: &Path, cwd: &Path, args: &[String]) -> Result<()> {
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
            .stdin(Stdio::null())
            .stdout(File::create(&stdout)?)
            .stderr(File::create(&stderr)?);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
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
        loop {
            if let Err(e) = self.cancel.check() {
                #[cfg(windows)]
                job.terminate()?;
                let _ = child.kill();
                child.wait()?;
                return Err(e);
            }
            if let Some(status) = child.try_wait()? {
                fs::write(stem.with_extension("exit.txt"), status.to_string())?;
                if !status.success() {
                    return Err(AppError::new(
                        "외부 도구가 실패했습니다. 도구 경로·원본과 상세 로그를 확인하세요.",
                        format!(
                            "{}\nexit: {status}\nstdout: {}\nstderr: {}\n{}",
                            exe.display(),
                            stdout.display(),
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
