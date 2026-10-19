//! Explicit, local integration runner. Input JSON has the same schema as the UI request.
use four_f_korean_patcher_lib::{
    model::{Cancel, Request},
    pipeline,
};
fn main() {
    let result = (|| {
        let path = std::env::args()
            .nth(1)
            .ok_or("request JSON path required")?;
        let request: Request = serde_json::from_slice(&std::fs::read(path)?)?;
        match pipeline::run(request, Cancel::default(), |p| {
            println!("{}", serde_json::to_string(&p).unwrap())
        }) {
            Ok(out) => {
                println!("{}", serde_json::to_string(&out)?);
                Ok(())
            }
            Err(e) => {
                eprintln!("{}", serde_json::to_string_pretty(&e)?);
                Err("patch failed".into())
            }
        }
    })() as Result<(), Box<dyn std::error::Error>>;
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
