use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub base: PathBuf,
    pub patch1: PathBuf,
    pub patch2: PathBuf,
    pub dlc: Option<PathBuf>,
    pub dlc_patch: Option<PathBuf>,
    pub toolkit: PathBuf,
    pub decryptor: PathBuf,
    pub work: PathBuf,
    pub output: PathBuf,
    pub format: Format,
}
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Cia,
    ThreeDs,
}
impl Format {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Cia => "cia",
            Self::ThreeDs => "3ds",
        }
    }
}
#[derive(Default, Clone)]
pub struct Cancel(pub Arc<AtomicBool>);
impl Cancel {
    pub fn check(&self) -> Result<()> {
        if self.0.load(Ordering::SeqCst) {
            Err(AppError::cancelled())
        } else {
            Ok(())
        }
    }
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub run_id: String,
    pub stage: String,
    pub completed: usize,
    pub total: usize,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub files: Vec<String>,
    pub warning: Option<String>,
}
