//! Focused real-tool DLC check, using the UI request JSON. Retains outputs for inspection.
use four_f_korean_patcher_lib::{
    archive::{self, Layout},
    hash,
    model::{Cancel, Request},
    tools::{
        dlc::{self, Decryptor},
        runner::ProcessRunner,
        toolkit::Toolkit,
    },
    workspace,
};
fn main() {
    let result = (|| {
        let path = std::env::args().nth(1).ok_or("request JSON required")?;
        let request: Request = serde_json::from_slice(&std::fs::read(path)?)?;
        let original = request.dlc.ok_or("DLC input required")?;
        let patch = request.dlc_patch.ok_or("DLC patch required")?;
        let cancel = Cancel::default();
        hash::verify(&original, hash::DLC, &cancel).map_err(|e| e.detail.to_string())?;
        let root = request
            .work
            .join(format!("dlc-probe-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root)?;
        std::fs::create_dir(root.join("logs"))?;
        let run = (|| {
            workspace::copy(&original, &root.join("input.cia"), &cancel)?;
            let mut runner = ProcessRunner {
                cancel: cancel.clone(),
                logs: root.join("logs"),
                sequence: 0,
            };
            let toolkit = Toolkit::new(&request.toolkit)?;
            let decryptor = Decryptor::new(&request.decryptor)?;
            let catalog = decryptor.decrypt(&mut runner, &root)?;
            dlc::extract(&mut runner, &toolkit, &root, &catalog)?;
            archive::apply(&patch, &root, Layout::Patch, &cancel)?;
            let auxiliary = dlc::auxiliary(&patch, &root, &cancel)?;
            let output = decryptor.rebuild(&mut runner, &toolkit, &root, &catalog, auxiliary)?;
            println!(
                "DLC contents: {}; output: {}",
                catalog.contents.len(),
                output.display()
            );
            Ok::<_, four_f_korean_patcher_lib::error::AppError>(())
        })();
        run.map_err(|e| format!("{}\n{}\nworkspace: {}", e.message, e.detail, root.display()))?;
        Ok(())
    })() as Result<(), Box<dyn std::error::Error>>;
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
