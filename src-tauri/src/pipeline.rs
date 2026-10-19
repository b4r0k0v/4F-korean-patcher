use crate::{
    archive::{self, Layout},
    error::Result,
    hash,
    model::{Cancel, Outcome, Progress, Request},
    tools::{
        dlc::{self, Decryptor},
        runner::ProcessRunner,
        toolkit::Toolkit,
    },
    validation,
    workspace::{self, Workspace},
};

pub fn run(request: Request, cancel: Cancel, emit: impl Fn(Progress)) -> Result<Outcome> {
    let id = uuid::Uuid::new_v4().to_string();
    let mut stage = String::from("사전 검사");
    let mut completed = 0;
    let mut workspace = None;
    let mut saved = vec![];
    let result = (|| {
        let mut step = |name: &str| -> Result<()> {
            cancel.check()?;
            stage = name.into();
            emit(Progress {
                run_id: id.clone(),
                stage: stage.clone(),
                completed,
                total: if request.dlc.is_some() { 12 } else { 8 },
            });
            completed += 1;
            Ok(())
        };
        step("입력·도구·원본 검사")?;
        validation::validate(&request)?;
        hash::verify(&request.base, hash::BASE, &cancel)?;
        if let Some(p) = &request.dlc {
            hash::verify(p, hash::DLC, &cancel)?;
        }
        let w = Workspace::create(&request, &id)?;
        workspace = Some(w);
        let w = workspace.as_ref().unwrap();
        let base = w.root.join("base");
        step("원본·패치 복사")?;
        workspace::copy(&request.base, &base.join("input.3ds"), &cancel)?;
        hash::verify(&base.join("input.3ds"), hash::BASE, &cancel)?;
        for (p, n) in [
            (&request.patch1, "first.zip"),
            (&request.patch2, "second.zip"),
        ] {
            workspace::copy(p, &w.root.join("patches").join(n), &cancel)?;
        }
        let toolkit = Toolkit::new(&request.toolkit)?;
        let mut runner = ProcessRunner {
            cancel: cancel.clone(),
            logs: w.root.join("logs"),
            sequence: 0,
        };
        let decryptor = Decryptor::new(&request.decryptor)?;
        let dlc_folder = w.root.join("dlc");
        let catalog = if let (Some(original), Some(patch)) = (&request.dlc, &request.dlc_patch) {
            workspace::copy(original, &dlc_folder.join("input.cia"), &cancel)?;
            hash::verify(&dlc_folder.join("input.cia"), hash::DLC, &cancel)?;
            workspace::copy(patch, &w.root.join("patches/dlc.zip"), &cancel)?;
            step("DLC 해독")?;
            Some(decryptor.decrypt(&mut runner, &dlc_folder)?)
        } else {
            None
        };
        step("본편 추출")?;
        toolkit.extract_base(&mut runner, &base)?;
        step("본편 1차 패치 적용")?;
        archive::apply(
            &w.root.join("patches/first.zip"),
            &base,
            Layout::First,
            &cancel,
        )?;
        step("본편 2차 패치 적용")?;
        archive::apply(
            &w.root.join("patches/second.zip"),
            &base,
            Layout::Patch,
            &cancel,
        )?;
        let mut auxiliary = false;
        if let Some(catalog) = &catalog {
            step("DLC 전체 콘텐츠 추출")?;
            dlc::extract(&mut runner, &toolkit, &dlc_folder, catalog)?;
            step("DLC 패치 적용")?;
            archive::apply(
                &w.root.join("patches/dlc.zip"),
                &dlc_folder,
                Layout::Patch,
                &cancel,
            )?;
            auxiliary = dlc::auxiliary(&w.root.join("patches/dlc.zip"), &dlc_folder, &cancel)?;
        }
        step("본편 재생성")?;
        let output = toolkit.rebuild_base(&mut runner, &base, request.format)?;
        let dlc_output = if let Some(catalog) = &catalog {
            step("DLC 전체 콘텐츠 재생성")?;
            Some(decryptor.rebuild(&mut runner, &toolkit, &dlc_folder, catalog, auxiliary)?)
        } else {
            None
        };
        step("결과 저장·복사 검증")?;
        let out = workspace::publish(
            &output,
            &request.output,
            &format!("SMT4F-KO-{id}.{}", request.format.extension()),
            &cancel,
        )?;
        saved.push(out.to_string_lossy().into_owned());
        if let Some(output) = dlc_output {
            let out = workspace::publish(
                &output,
                &request.output,
                &format!("SMT4F-KO-DLC-{id}.cia"),
                &cancel,
            )?;
            saved.push(out.to_string_lossy().into_owned());
        }
        step("임시 작업 폴더 정리")?;
        let warning = w.cleanup().err().map(|e| {
            format!(
                "결과 생성 성공. 임시 폴더 정리 실패: {}\n{}",
                w.root.display(),
                e.detail
            )
        });
        Ok(Outcome {
            files: saved.clone(),
            warning,
        })
    })();
    result.map_err(|mut e: crate::error::AppError| {
        e.run_id = Some(id);
        e.stage = Some(stage);
        e.saved_files = saved;
        if let Some(w) = workspace {
            e.workspace = Some(w.root.to_string_lossy().into_owned());
            let _ = std::fs::write(
                w.root.join("error.json"),
                serde_json::to_vec_pretty(&e).unwrap_or_default(),
            );
        }
        e
    })
}
