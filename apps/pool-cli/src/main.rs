//! `pool-cli`: a stateless JSON CLI around the Stellar Private Payments SDK.
//! The Shield API calls it as a subprocess for everything that needs the
//! prover (the SDK cannot be linked next to `sqlx`). See [`command`] for the
//! protocol.

mod command;
mod pool;

use std::{io::Write, process::ExitCode};

use command::CliError;

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let outcome = match command::parse(&args) {
        Ok(command) => pool::run(command).await,
        Err(err) => Err(err),
    };
    let (body, code) = match outcome {
        Ok(value) => (value, ExitCode::SUCCESS),
        Err(err) => (error_json(&err), ExitCode::FAILURE),
    };
    let mut stdout = std::io::stdout().lock();
    // The caller reads exactly one JSON line; nothing else goes to stdout.
    if writeln!(stdout, "{body}").is_err() {
        return ExitCode::FAILURE;
    }
    code
}

fn error_json(err: &CliError) -> serde_json::Value {
    serde_json::to_value(err)
        .unwrap_or_else(|_| serde_json::json!({"error": "internal", "retryable": false}))
}
