use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub message: String,
    pub detail: Box<str>,
    pub run_id: Option<String>,
    pub stage: Option<Box<str>>,
    pub workspace: Option<Box<str>>,
    pub saved_files: Vec<String>,
}
impl AppError {
    pub fn new(message: impl Into<String>, detail: impl ToString) -> Self {
        Self {
            message: message.into(),
            detail: detail.to_string().into_boxed_str(),
            run_id: None,
            stage: None,
            workspace: None,
            saved_files: vec![],
        }
    }
    pub fn cancelled() -> Self {
        Self::new("작업을 취소했습니다. 작업 폴더를 보존합니다.", "cancelled")
    }
}
impl<E: std::error::Error> From<E> for AppError {
    fn from(e: E) -> Self {
        Self::new(
            "파일 또는 도구 처리에 실패했습니다. 경로·권한·여유 공간을 확인하세요.",
            e,
        )
    }
}
pub type Result<T> = std::result::Result<T, AppError>;
