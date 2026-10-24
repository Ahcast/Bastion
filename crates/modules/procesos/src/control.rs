use std::{io, path::PathBuf};

use bastion_core::CoreError;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default)]
pub struct ScoutControl {
    socket: Option<PathBuf>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScoutControlStatus {
    pub available: bool,
    pub busy: bool,
    pub job: Option<ScoutJob>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScoutJob {
    pub id: String,
    pub status: ScoutJobStatus,
    pub started_at: String,
    pub finished_at: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScoutJobStatus {
    Running,
    Succeeded,
    Partial,
    Failed,
    Busy,
    Interrupted,
}

pub fn valid_scout_key(key: &str) -> bool {
    (16..=128).contains(&key.len())
        && key
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
}

impl ScoutControl {
    pub fn from_env() -> io::Result<Self> {
        let socket = std::env::var_os("BASTION_SCOUT_CONTROL_SOCKET")
            .filter(|path| !path.is_empty())
            .map(PathBuf::from);
        if socket.as_ref().is_some_and(|path| !path.is_absolute()) {
            return Err(io::Error::other("Scout control socket must be absolute"));
        }
        Ok(Self { socket })
    }

    pub fn unavailable() -> ScoutControlStatus {
        ScoutControlStatus {
            available: false,
            busy: false,
            job: None,
        }
    }

    pub fn status(&self) -> ScoutControlStatus {
        self.call(serde_json::json!({"action":"status"}))
            .unwrap_or_else(|_| Self::unavailable())
    }

    pub fn start(&self, actor: u64, key: &str) -> Result<ScoutControlStatus, CoreError> {
        if !valid_scout_key(key) {
            return Err(CoreError::Invalid);
        }
        self.call(serde_json::json!({"action":"start","id":format!("{actor}_{key}")}))
    }

    fn call(&self, request: serde_json::Value) -> Result<ScoutControlStatus, CoreError> {
        let path = self.socket.as_ref().ok_or(CoreError::Overloaded)?;
        #[cfg(unix)]
        {
            use std::io::{BufRead, BufReader, Read, Write};
            use std::os::unix::net::UnixStream;
            let mut stream = UnixStream::connect(path).map_err(|_| CoreError::Overloaded)?;
            let timeout = Some(std::time::Duration::from_secs(3));
            stream
                .set_read_timeout(timeout)
                .map_err(|_| CoreError::Overloaded)?;
            stream
                .set_write_timeout(timeout)
                .map_err(|_| CoreError::Overloaded)?;
            let mut bytes = serde_json::to_vec(&request).map_err(|_| CoreError::Invalid)?;
            bytes.push(b'\n');
            stream
                .write_all(&bytes)
                .map_err(|_| CoreError::Overloaded)?;
            let mut reply = String::new();
            BufReader::new(stream.take(16_385))
                .read_line(&mut reply)
                .map_err(|_| CoreError::Overloaded)?;
            if reply.len() > 16_384 || !reply.ends_with('\n') {
                return Err(CoreError::Overloaded);
            }
            let value: serde_json::Value =
                serde_json::from_str(&reply).map_err(|_| CoreError::Overloaded)?;
            if let Some(error) = value.get("error").and_then(|v| v.as_str()) {
                return Err(match error {
                    "busy" => CoreError::Conflict,
                    "invalid" => CoreError::Invalid,
                    _ => CoreError::Overloaded,
                });
            }
            serde_json::from_value(value).map_err(|_| CoreError::Overloaded)
        }
        #[cfg(not(unix))]
        {
            let _ = (path, request);
            Err(CoreError::Overloaded)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn disabled_control_and_unsafe_keys_never_start_a_process() {
        let control = ScoutControl::default();
        assert!(!control.status().available);
        for key in [
            "short",
            "../../etc/passwd",
            "a key with spaces",
            "0123456789abcdef;sh",
        ] {
            assert!(!valid_scout_key(key));
            assert!(matches!(control.start(1, key), Err(CoreError::Invalid)));
        }
        assert!(valid_scout_key("1234567890abcdef-abc_DEF"));
        assert!(control.start(1, "1234567890abcdef").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn private_socket_uses_a_fixed_protocol_and_maps_busy_without_echoing_output() {
        use std::io::{BufRead, BufReader, Write};
        use std::os::unix::net::UnixListener;
        let path = std::env::temp_dir().join(format!(
            "bastion-scout-{}-{}.sock",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        let listener = UnixListener::bind(&path).unwrap();
        let worker = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut input = String::new();
            BufReader::new(stream.try_clone().unwrap())
                .read_line(&mut input)
                .unwrap();
            let input: serde_json::Value = serde_json::from_str(&input).unwrap();
            assert_eq!(
                input,
                serde_json::json!({"action":"start","id":"7_1234567890abcdef"})
            );
            stream.write_all(b"{\"error\":\"busy\"}\n").unwrap();
        });
        let control = ScoutControl {
            socket: Some(path.clone()),
        };
        assert!(matches!(
            control.start(7, "1234567890abcdef"),
            Err(CoreError::Conflict)
        ));
        worker.join().unwrap();
        std::fs::remove_file(path).unwrap();
    }
}
