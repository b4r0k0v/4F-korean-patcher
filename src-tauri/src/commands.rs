use crate::{
    error::{AppError, Result},
    hash,
    model::{Cancel, Request},
    validation,
};

#[tauri::command]
pub async fn check_inputs(request: Request) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || validation::validate(&request))
        .await
        .map_err(|e| AppError::new("입력 검사 실패", e))?
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
