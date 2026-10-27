//! Manual, explicitly budgeted synthetic smoke. Never invoked by tests or the app.
use spacetop_core::summary::{adapters, digest, output, process, prompt, SummaryProvider};
use std::sync::atomic::AtomicBool;
fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    anyhow::ensure!(
        (4..=5).contains(&args.len()),
        "provider source-file private-output-directory"
    );
    let provider = match args[1].as_str() {
        "claude" => SummaryProvider::Claude,
        "codex" => SummaryProvider::Codex,
        _ => anyhow::bail!("unknown provider"),
    };
    let input = std::fs::read(&args[2])?;
    anyhow::ensure!(input.len() <= 8192, "synthetic smoke limit");
    let entity = spacetop_core::parser::parse_work_item(
        std::path::Path::new(&args[2]),
        &["implement".into()],
        None,
    )?;
    let snapshot = spacetop_core::summary::snapshot::capture(
        std::path::Path::new(&args[2]).parent().unwrap(),
        &entity,
        provider,
    )?;
    anyhow::ensure!(
        snapshot.input.len() <= 8192,
        "assembled synthetic smoke limit"
    );
    let prompt = prompt::build(&snapshot.input)?;
    let outdir = std::path::Path::new(&args[3]);
    anyhow::ensure!(
        outdir.is_absolute() && outdir.starts_with("/tmp"),
        "private /tmp output required"
    );
    std::fs::create_dir_all(outdir)?;
    std::fs::write(outdir.join("prompt.txt"), &prompt)?;
    if args.get(4).is_some_and(|mode| mode == "prepare") {
        let identity = serde_json::json!({"provider": provider, "model": provider.model(), "argv": adapters::arguments(provider), "source_sha256": digest(&input), "input_sha256": digest(&snapshot.input), "input_bytes": snapshot.input.len(), "prompt_sha256": digest(prompt.as_bytes()), "prompt_bytes": prompt.len()});
        std::fs::write(
            outdir.join("preparation.json"),
            serde_json::to_vec_pretty(&identity)?,
        )?;
        println!("{}", serde_json::to_string_pretty(&identity)?);
        return Ok(());
    }
    let scratch = process::Scratch::new()?;
    let mut command = adapters::command(provider);
    command.current_dir(scratch.path());
    let outcome = process::run(
        command,
        prompt.as_bytes(),
        &AtomicBool::new(false),
        process::Limits::default(),
    );
    let result = match outcome {
        Ok(raw) => {
            std::fs::write(outdir.join("stdout.bin"), &raw.stdout)?;
            std::fs::write(outdir.join("stderr.bin"), &raw.stderr)?;
            let parsed = if raw.success {
                output::parse(provider, &raw.stdout)
            } else {
                Err(spacetop_core::summary::SummaryError::Provider)
            };
            if let Ok(ref text) = parsed {
                std::fs::write(outdir.join("summary.txt"), text)?;
            }
            serde_json::json!({"exit_success": raw.success, "stdout_sha256": digest(&raw.stdout), "stderr_sha256": digest(&raw.stderr), "stdout_bytes": raw.stdout.len(), "stderr_bytes": raw.stderr.len(), "parsed": parsed.as_ref().map(|text| digest(text.as_bytes())).map_err(ToString::to_string)})
        }
        Err(error) => serde_json::json!({"process_error": error.to_string()}),
    };
    let evidence = serde_json::json!({"provider": provider, "model": provider.model(), "argv": adapters::arguments(provider), "source_sha256": digest(&input), "input_sha256": digest(&snapshot.input), "prompt_sha256": digest(prompt.as_bytes()), "result": result});
    std::fs::write(
        outdir.join("result.json"),
        serde_json::to_vec_pretty(&evidence)?,
    )?;
    println!("{}", serde_json::to_string_pretty(&evidence)?);
    Ok(())
}
