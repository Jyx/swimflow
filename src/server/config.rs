//! `.env`-driven configuration for `swimflow --serve`.
//!
//! Precedence is: defaults < `.env` file < process environment. The `.env`
//! file is `./.env` by default (optional), or the path given by `--env-file`
//! or `SWIMFLOW_ENV` (which must exist). Values are resolved relative to the
//! current working directory.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::text::FontSpec;

use super::ServerError;

/// Default bind address: every interface, so other machines can connect.
const DEFAULT_HOST: &str = "0.0.0.0";
const DEFAULT_PORT: u16 = 8000;
const DEFAULT_DEBOUNCE_MS: u64 = 600;
const DEFAULT_SCALE: f32 = 1.0;

/// Resolved settings for the live-preview server.
#[derive(Debug, Clone)]
pub struct ServeConfig {
    /// Directory scanned recursively for `.diag` files (canonicalized).
    pub dir: PathBuf,
    /// Interface to bind (`0.0.0.0` by default).
    pub host: String,
    /// TCP port to bind.
    pub port: u16,
    /// Re-render delay after the last keystroke, in milliseconds.
    pub debounce_ms: u64,
    /// Raster scale for PNG downloads (1.0 = 96 DPI).
    pub scale: f32,
    /// Optional shared secret. When set, requests need `?token=…`, a
    /// `Bearer` header or the `swimflow_token` cookie.
    pub token: Option<String>,
    /// Font used for preview rendering and served at [`super::FONT_URL`].
    pub font: FontSpec,
}

impl ServeConfig {
    /// Resolve the configuration from the environment and `.env` file.
    pub fn resolve(env_file: Option<&Path>) -> Result<Self, ServerError> {
        let values = load_env_file(env_file)?;
        Self::from_values(&values, |key| std::env::var(key).ok())
    }

    /// Resolve from explicit file values plus a process-environment lookup.
    ///
    /// Split out (and private) so unit tests can supply values without
    /// mutating the process environment, which is shared between tests.
    fn from_values(
        file: &BTreeMap<String, String>,
        lookup: impl Fn(&str) -> Option<String>,
    ) -> Result<Self, ServerError> {
        let get = |key: &str| lookup(key).or_else(|| file.get(key).cloned());

        let dir_value = get("SWIMFLOW_DIR")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| ".".to_string());
        let dir = std::fs::canonicalize(&dir_value).map_err(|err| {
            ServerError::Config(format!(
                "SWIMFLOW_DIR {dir_value:?} is not accessible: {err}"
            ))
        })?;
        if !dir.is_dir() {
            return Err(ServerError::Config(format!(
                "SWIMFLOW_DIR {dir_value:?} is not a directory"
            )));
        }

        let host = get("SWIMFLOW_HOST")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| DEFAULT_HOST.to_string());

        let port = match get("SWIMFLOW_PORT") {
            Some(value) => value
                .trim()
                .parse::<u16>()
                .map_err(|_| ServerError::Config(format!("invalid SWIMFLOW_PORT: {value:?}")))?,
            None => DEFAULT_PORT,
        };

        let debounce_ms = match get("SWIMFLOW_DEBOUNCE_MS") {
            Some(value) => value.trim().parse::<u64>().map_err(|_| {
                ServerError::Config(format!("invalid SWIMFLOW_DEBOUNCE_MS: {value:?}"))
            })?,
            None => DEFAULT_DEBOUNCE_MS,
        };

        let scale = match get("SWIMFLOW_SCALE") {
            Some(value) => {
                let scale = value.trim().parse::<f32>().map_err(|_| {
                    ServerError::Config(format!("invalid SWIMFLOW_SCALE: {value:?}"))
                })?;
                if !scale.is_finite() || scale <= 0.0 {
                    return Err(ServerError::Config(format!(
                        "invalid SWIMFLOW_SCALE: {value:?} (expected a positive number)"
                    )));
                }
                scale
            }
            None => DEFAULT_SCALE,
        };

        let token = get("SWIMFLOW_TOKEN")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        let font_path = get("SWIMFLOW_FONT")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let font_family = get("SWIMFLOW_FONT_FAMILY")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let font = FontSpec::resolve(font_path.as_deref().map(Path::new), font_family.as_deref())
            .map_err(|err| ServerError::Config(err.to_string()))?;

        Ok(ServeConfig {
            dir,
            host,
            port,
            debounce_ms,
            scale,
            token,
            font,
        })
    }
}

/// Read the `.env` file (or use an empty map when the default is absent).
fn load_env_file(explicit: Option<&Path>) -> Result<BTreeMap<String, String>, ServerError> {
    let path = match explicit {
        Some(path) => Some(path.to_path_buf()),
        None => std::env::var_os("SWIMFLOW_ENV").map(PathBuf::from),
    };

    match path {
        Some(path) => {
            let text = std::fs::read_to_string(&path).map_err(|err| {
                ServerError::Config(format!("cannot read env file {}: {err}", path.display()))
            })?;
            Ok(parse_env(&text))
        }
        None => {
            let default = Path::new(".env");
            if default.is_file() {
                Ok(parse_env(&std::fs::read_to_string(default)?))
            } else {
                Ok(BTreeMap::new())
            }
        }
    }
}

/// Parse a minimal `.env` dialect: `KEY=VALUE` lines, `#` comments, optional
/// `export ` prefix, and single/double quoted values. Unknown lines are
/// ignored rather than treated as errors.
pub fn parse_env(text: &str) -> BTreeMap<String, String> {
    let mut values = BTreeMap::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line).trim_start();
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        values.insert(key.to_string(), unquote(value.trim()).to_string());
    }
    values
}

/// Strip matching surrounding quotes, or a trailing ` # comment` on an
/// unquoted value.
fn unquote(value: &str) -> &str {
    let value = value.trim();
    let quoted = value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')));
    if quoted {
        &value[1..value.len() - 1]
    } else if let Some(index) = value.find(" #") {
        value[..index].trim_end()
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> crate::test_support::TempDir {
        crate::test_support::TempDir::new(&format!("swimflow-server-config-{name}-"))
    }

    #[test]
    fn parses_comments_quotes_and_exports() {
        let values = parse_env(
            "# comment\n\
             SWIMFLOW_DIR=examples\n\
             export SWIMFLOW_PORT = 9000\n\
             SWIMFLOW_HOST=\"127.0.0.1\"\n\
             SWIMFLOW_TOKEN='s3cret'\n\
             EMPTY\n",
        );
        assert_eq!(values.get("SWIMFLOW_DIR").unwrap(), "examples");
        assert_eq!(values.get("SWIMFLOW_PORT").unwrap(), "9000");
        assert_eq!(values.get("SWIMFLOW_HOST").unwrap(), "127.0.0.1");
        assert_eq!(values.get("SWIMFLOW_TOKEN").unwrap(), "s3cret");
        assert!(!values.contains_key("EMPTY"));
    }

    #[test]
    fn strips_inline_comments_from_unquoted_values() {
        let values = parse_env("SWIMFLOW_HOST=0.0.0.0 # all interfaces\n");
        assert_eq!(values.get("SWIMFLOW_HOST").unwrap(), "0.0.0.0");
    }

    #[test]
    fn process_environment_wins_over_the_file() {
        let dir = temp_dir("precedence");
        let mut file = BTreeMap::new();
        file.insert("SWIMFLOW_DIR".to_string(), dir.display().to_string());
        file.insert("SWIMFLOW_PORT".to_string(), "1234".to_string());

        let config = ServeConfig::from_values(&file, |key| {
            (key == "SWIMFLOW_PORT").then(|| "5678".to_string())
        })
        .unwrap();
        assert_eq!(config.port, 5678);
        assert_eq!(config.host, DEFAULT_HOST);
        assert_eq!(config.debounce_ms, DEFAULT_DEBOUNCE_MS);
        assert_eq!(config.scale, DEFAULT_SCALE);
        assert!(config.token.is_none());
    }

    #[test]
    fn parses_the_png_scale() {
        let dir = temp_dir("scale");
        let mut file = BTreeMap::new();
        file.insert("SWIMFLOW_DIR".to_string(), dir.display().to_string());
        file.insert("SWIMFLOW_SCALE".to_string(), "2.5".to_string());
        let config = ServeConfig::from_values(&file, |_| None).unwrap();
        assert_eq!(config.scale, 2.5);
    }

    #[test]
    fn invalid_scale_is_rejected() {
        let dir = temp_dir("scale-bad");
        for value in ["large", "0", "-2", "inf", "NaN"] {
            let mut file = BTreeMap::new();
            file.insert("SWIMFLOW_DIR".to_string(), dir.display().to_string());
            file.insert("SWIMFLOW_SCALE".to_string(), value.to_string());
            let err = ServeConfig::from_values(&file, |_| None).unwrap_err();
            assert!(err.to_string().contains("SWIMFLOW_SCALE"), "{value}: {err}");
        }
    }

    #[test]
    fn invalid_port_is_rejected() {
        let dir = temp_dir("port");
        let mut file = BTreeMap::new();
        file.insert("SWIMFLOW_DIR".to_string(), dir.display().to_string());
        file.insert("SWIMFLOW_PORT".to_string(), "not-a-port".to_string());
        let err = ServeConfig::from_values(&file, |_| None).unwrap_err();
        assert!(err.to_string().contains("SWIMFLOW_PORT"), "{err}");
    }

    #[test]
    fn missing_directory_is_rejected() {
        let mut file = BTreeMap::new();
        file.insert(
            "SWIMFLOW_DIR".to_string(),
            "/definitely/not/a/real/swimflow/dir".to_string(),
        );
        let err = ServeConfig::from_values(&file, |_| None).unwrap_err();
        assert!(err.to_string().contains("SWIMFLOW_DIR"), "{err}");
    }

    #[test]
    fn resolves_the_embedded_font_by_default() {
        let dir = temp_dir("font-default");
        let mut file = BTreeMap::new();
        file.insert("SWIMFLOW_DIR".to_string(), dir.display().to_string());
        let config = ServeConfig::from_values(&file, |_| None).unwrap();
        assert_eq!(config.font.family, crate::text::DEFAULT_FAMILY);

        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/assets/default.ttf");
        file.insert("SWIMFLOW_FONT".to_string(), path.display().to_string());
        let config = ServeConfig::from_values(&file, |_| None).unwrap();
        assert_eq!(config.font.family, crate::text::DEFAULT_FAMILY);
    }

    #[test]
    fn invalid_font_path_is_rejected() {
        let dir = temp_dir("font-bad");
        let mut file = BTreeMap::new();
        file.insert("SWIMFLOW_DIR".to_string(), dir.display().to_string());
        file.insert(
            "SWIMFLOW_FONT".to_string(),
            "/no/such/swimflow-font.ttf".to_string(),
        );
        let err = ServeConfig::from_values(&file, |_| None).unwrap_err();
        assert!(err.to_string().contains("cannot read font"), "{err}");
    }

    #[test]
    fn blank_token_means_no_auth() {
        let dir = temp_dir("token");
        let mut file = BTreeMap::new();
        file.insert("SWIMFLOW_DIR".to_string(), dir.display().to_string());
        file.insert("SWIMFLOW_TOKEN".to_string(), "   ".to_string());
        let config = ServeConfig::from_values(&file, |_| None).unwrap();
        assert!(config.token.is_none());
    }
}
