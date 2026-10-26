//! SEP-1 `stellar.toml`: only the top-level string fields a SEP-24 client
//! needs. Spec: <https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0001.md>.
//!
//! A deliberately small parser: top-level `KEY = "value"` lines before the
//! first `[table]`. Arrays, tables and other types are skipped, which is
//! enough for `SIGNING_KEY`, `WEB_AUTH_ENDPOINT`,
//! `TRANSFER_SERVER_SEP0024` and `NETWORK_PASSPHRASE`.

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StellarToml {
    pub signing_key: String,
    pub web_auth_endpoint: String,
    pub transfer_server_sep0024: String,
    pub network_passphrase: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TomlError {
    #[error("stellar.toml has no {0}")]
    Missing(&'static str),
    #[error("stellar.toml line {0} is malformed")]
    Malformed(usize),
}

/// Parses the fields above from a `stellar.toml` document.
pub fn parse(document: &str) -> Result<StellarToml, TomlError> {
    let mut fields = HashMap::new();
    for (index, line) in document.lines().enumerate() {
        let line = line.trim();
        if line.starts_with('[') {
            break; // only top-level keys
        }
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            return Err(TomlError::Malformed(index + 1));
        };
        let value = value.trim();
        if let Some(string) = value.strip_prefix('"').and_then(|v| v.strip_suffix('"')) {
            if string.contains('"') || string.contains('\\') {
                return Err(TomlError::Malformed(index + 1));
            }
            fields.insert(key.trim().to_owned(), string.to_owned());
        }
    }
    let mut take = |key: &'static str| fields.remove(key).ok_or(TomlError::Missing(key));
    Ok(StellarToml {
        signing_key: take("SIGNING_KEY")?,
        web_auth_endpoint: take("WEB_AUTH_ENDPOINT")?,
        transfer_server_sep0024: take("TRANSFER_SERVER_SEP0024")?,
        network_passphrase: take("NETWORK_PASSPHRASE")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Lendara sandbox's real stellar.toml (26-oct-2026).
    const SANDBOX: &str = r#"VERSION = "2.7.0"
NETWORK_PASSPHRASE = "Test SDF Network ; September 2015"
SIGNING_KEY = "GDG7AXFMRFVJYN7XCDFPL2JSDWDYBJ32FFBLHVX6N4KXLQW2NOHQTBUA"
WEB_AUTH_ENDPOINT = "https://lendaraweb-production.up.railway.app/auth"
TRANSFER_SERVER_SEP0024 = "https://lendaraweb-production.up.railway.app/sep24"
ACCOUNTS = ["GAO7YFNVZAHK6RSKV2LNPIS3WDGCHLBRLIFQETTPE36TLFFAGENNMNT5"]

[DOCUMENTATION]
  ORG_NAME = "Lendara (Sandbox)"
  SIGNING_KEY = "GIGNORED"
"#;

    #[test]
    fn parses_the_sandbox_toml() -> Result<(), TomlError> {
        let toml = parse(SANDBOX)?;
        assert_eq!(
            toml.signing_key,
            "GDG7AXFMRFVJYN7XCDFPL2JSDWDYBJ32FFBLHVX6N4KXLQW2NOHQTBUA"
        );
        assert_eq!(
            toml.web_auth_endpoint,
            "https://lendaraweb-production.up.railway.app/auth"
        );
        assert_eq!(
            toml.transfer_server_sep0024,
            "https://lendaraweb-production.up.railway.app/sep24"
        );
        assert_eq!(toml.network_passphrase, "Test SDF Network ; September 2015");
        Ok(())
    }

    #[test]
    fn keys_inside_tables_are_ignored() {
        let doc = "[DOCUMENTATION]\nSIGNING_KEY = \"G\"\n";
        assert_eq!(parse(doc), Err(TomlError::Missing("SIGNING_KEY")));
    }

    #[test]
    fn reports_missing_fields() {
        let doc = SANDBOX.replace("TRANSFER_SERVER_SEP0024", "TRANSFER_SERVER");
        assert_eq!(
            parse(&doc),
            Err(TomlError::Missing("TRANSFER_SERVER_SEP0024"))
        );
    }

    #[test]
    fn rejects_malformed_lines_and_escapes() {
        assert_eq!(parse("NOT A KEY VALUE\n"), Err(TomlError::Malformed(1)));
        assert_eq!(
            parse("SIGNING_KEY = \"a\\\"b\"\n"),
            Err(TomlError::Malformed(1))
        );
    }
}
