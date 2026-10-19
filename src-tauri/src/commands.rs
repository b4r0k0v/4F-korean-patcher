use crate::{
    error::{AppError, Result},
    hash,
    model::{Cancel, Request},
    validation,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use tauri::{Emitter, Manager};

#[derive(Default)]
pub struct Jobs {
    pub active: Mutex<Option<Cancel>>,
    pub exit_after_cancel: AtomicBool,
}

#[tauri::command]
pub async fn start_patch(app: tauri::AppHandle, request: Request) -> Result<crate::model::Outcome> {
    let cancel = Cancel::default();
    {
        let state = app.state::<Jobs>();
        let mut active = state
            .active
            .lock()
            .map_err(|e| AppError::new("작업 상태 확인 실패", e))?;
        if active.is_some() {
            return Err(AppError::new(
                "이미 패치 작업을 실행하고 있습니다.",
                "one job at a time",
            ));
        }
        *active = Some(cancel.clone());
    }
    let events = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        crate::pipeline::run(request, cancel, |p| {
            let _ = events.emit("patch-progress", p);
        })
    })
    .await
    .map_err(|e| AppError::new("작업 스레드가 종료되었습니다.", e))
    .and_then(|r| r);
    let state = app.state::<Jobs>();
    *state
        .active
        .lock()
        .map_err(|e| AppError::new("작업 상태 갱신 실패", e))? = None;
    if state.exit_after_cancel.load(Ordering::SeqCst) {
        app.exit(0);
    }
    result
}
#[tauri::command]
pub fn cancel_patch(app: tauri::AppHandle, close: bool) -> Result<()> {
    let state = app.state::<Jobs>();
    let active = state
        .active
        .lock()
        .map_err(|e| AppError::new("취소 요청 실패", e))?;
    if close {
        state.exit_after_cancel.store(true, Ordering::SeqCst);
    }
    if let Some(c) = active.as_ref() {
        c.cancel();
    } else if close {
        app.exit(0);
    }
    Ok(())
}

#[tauri::command]
pub async fn check_inputs(request: Request) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || validation::validate(&request))
        .await
        .map_err(|e| AppError::new("입력 검사 실패", e))?
}
#[tauri::command]
pub fn open_output(app: tauri::AppHandle, path: String) -> Result<()> {
    use tauri_plugin_opener::OpenerExt;
    let path = std::path::Path::new(&path);
    if !path.is_dir() {
        return Err(AppError::new(
            "출력 폴더를 찾을 수 없습니다.",
            path.display(),
        ));
    }
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)?;
    Ok(())
}
#[tauri::command]
pub async fn check_field(path: String, kind: String) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let p = std::path::Path::new(&path);
        match kind.as_str() {
            "toolkit" => {
                validation::executable(p, &["3dstool.exe"])?;
                validation::ctr(p)?;
                validation::makerom(p)?;
                Ok(())
            }
            "decryptor" => {
                for name in ["decrypt.exe", "ctrtool.exe", "makerom.exe"] {
                    validation::executable(p, &[name])?;
                }
                Ok(())
            }
            "dir" => validation::writable(p),
            "3ds" | "cia" | "zip" => validation::input(p, &kind),
            "first" | "patch" => {
                validation::input(p, "zip")?;
                crate::archive::validate(
                    p,
                    if kind == "first" {
                        crate::archive::Layout::First
                    } else {
                        crate::archive::Layout::Patch
                    },
                    &Cancel::default(),
                )
            }
            _ => Err(AppError::new("지원하지 않는 입력 형식입니다.", kind)),
        }
    })
    .await
    .map_err(|e| AppError::new("경로 검사 실패", e))?
}
#[tauri::command]
pub async fn check_hash(path: String, dlc: bool) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || {
        hash::verify(
            std::path::Path::new(&path),
            if dlc { hash::DLC } else { hash::BASE },
            &Cancel::default(),
        )
    })
    .await
    .map_err(|e| AppError::new("해시 검사 실패", e))?
}
