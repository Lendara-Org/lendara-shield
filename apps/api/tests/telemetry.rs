//! The tracing subscriber is process-global, so this lives in its own test
//! binary (each file under `tests/` is a separate process).

use shield_api::{config::LogFormat, telemetry};

#[test]
fn installs_once_and_refuses_a_second_subscriber() {
    assert!(telemetry::init(LogFormat::Json).is_ok());
    assert!(telemetry::init(LogFormat::Pretty).is_err());
}
