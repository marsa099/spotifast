//! Click-only bridge to the portable fork updater. Never installs upstream
//! binaries as a fallback when a fork integration cannot be verified.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
#[cfg(target_os = "linux")]
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Snapshot {
    pub schema: u32,
    pub phase: String,
    pub message: String,
    pub version: String,
    pub session: String,
    pub pid: u32,
    pub updated_at: u64,
}

impl Snapshot {
    pub fn failed(message: impl Into<String>) -> Self {
        Self {
            schema: 1,
            phase: "failed".into(),
            message: message.into(),
            ..Self::default()
        }
    }

    pub fn active(&self) -> bool {
        matches!(
            self.phase.as_str(),
            "starting"
                | "checking"
                | "integrating"
                | "repairing"
                | "needs_input"
                | "building"
                | "installing"
        )
    }

    pub fn ready(&self) -> bool {
        self.phase == "ready"
    }

    fn validate(self) -> Result<Self, String> {
        if self.schema != 1
            || !matches!(
                self.phase.as_str(),
                "idle"
                    | "starting"
                    | "checking"
                    | "integrating"
                    | "repairing"
                    | "needs_input"
                    | "building"
                    | "ready"
                    | "installing"
                    | "installed"
                    | "failed"
            )
        {
            return Err("Unsupported fork updater state".into());
        }
        if self.message.len() > 16_000 || self.version.len() > 128 || self.session.len() > 128 {
            return Err("Fork updater state exceeds its field limits".into());
        }
        Ok(self)
    }
}

#[derive(Clone, Debug)]
pub enum Request {
    Start { version: String },
    Attach,
    Activate { pid: u32 },
}

#[derive(Debug)]
struct Location {
    helper: PathBuf,
    state: PathBuf,
    monitoring: AtomicBool,
}

#[derive(Clone, Debug)]
pub struct Client {
    location: Arc<Location>,
    pub status: Snapshot,
    pub pending: bool,
}

impl Client {
    /// Discovery is only a file lookup. It does not run a helper, check out
    /// source, connect to Pi, or start Docker/tmux.
    pub fn discover(state: &Path) -> Option<Self> {
        if !cfg!(target_os = "linux") {
            return None;
        }
        let helper = match std::env::var_os("SPOTIFAST_FORK_UPDATER") {
            Some(path) => PathBuf::from(path),
            None => std::env::current_exe()
                .ok()?
                .parent()?
                .join("spotifast-fork-updater"),
        };
        if !helper.is_absolute() || !helper.is_file() {
            return None;
        }
        Some(Self::at(helper, state.join("fork-updater")))
    }

    pub fn at(helper: PathBuf, state: PathBuf) -> Self {
        Self {
            location: Arc::new(Location {
                helper,
                state,
                monitoring: AtomicBool::new(false),
            }),
            status: Snapshot {
                schema: 1,
                phase: "idle".into(),
                ..Snapshot::default()
            },
            pending: false,
        }
    }

    pub fn execute(
        &self,
        request: &Request,
        notify: impl Fn(Snapshot) -> bool + Send + 'static,
    ) -> Result<(), String> {
        let mut command = Command::new(&self.location.helper);
        match request {
            Request::Start { version } => {
                command.arg("start").arg("--version").arg(version);
            }
            Request::Attach => {
                command.arg("attach");
            }
            Request::Activate { pid } => {
                command.arg("activate").arg("--pid").arg(pid.to_string());
            }
        }
        command.arg("--state-dir").arg(&self.location.state);
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        let mut child = command
            .spawn()
            .map_err(|error| format!("Couldn't start fork updater: {error}"))?;
        let deadline = Instant::now() + Duration::from_secs(90);
        let exit = loop {
            match child.try_wait().map_err(|error| error.to_string())? {
                Some(exit) => break exit,
                None if Instant::now() >= deadline => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("Fork updater control command timed out; open its terminal to inspect the worker".into());
                }
                None => std::thread::sleep(Duration::from_millis(30)),
            }
        };
        if !exit.success() {
            let mut message = String::new();
            if let Some(stderr) = child.stderr.take() {
                let _ = stderr.take(16_000).read_to_string(&mut message);
            }
            return Err(if message.trim().is_empty() {
                format!("Fork updater exited with {exit}")
            } else {
                message.trim().to_owned()
            });
        }
        if matches!(request, Request::Start { .. }) {
            notify(self.read_status()?);
            self.monitor(notify)?;
        }
        Ok(())
    }

    fn read_status(&self) -> Result<Snapshot, String> {
        let file = std::fs::File::open(self.location.state.join("state.json"))
            .map_err(|error| format!("Couldn't read fork updater status: {error}"))?;
        if file.metadata().map_err(|error| error.to_string())?.len() > 65_536 {
            return Err("Fork updater status is too large".into());
        }
        let snapshot: Snapshot = serde_json::from_reader(file.take(65_537))
            .map_err(|error| format!("Invalid fork updater status: {error}"))?;
        let snapshot = snapshot.validate()?;
        #[cfg(target_os = "linux")]
        if snapshot.active() {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let missing =
                snapshot.pid > 0 && !Path::new("/proc").join(snapshot.pid.to_string()).exists();
            let never_started = snapshot.pid == 0 && now.saturating_sub(snapshot.updated_at) > 30;
            if missing || never_started {
                return Err("The detached updater stopped unexpectedly. Open its terminal to see the recorded output.".into());
            }
        }
        Ok(snapshot)
    }

    fn monitor(&self, notify: impl Fn(Snapshot) -> bool + Send + 'static) -> Result<(), String> {
        if self.location.monitoring.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        let client = self.clone();
        let started = std::thread::Builder::new()
            .name("fork-update-status".into())
            .spawn(move || {
                let mut previous = None;
                let deadline = Instant::now() + Duration::from_secs(7_260);
                loop {
                    if Instant::now() >= deadline {
                        notify(Snapshot::failed("Updater status monitoring reached its two-hour limit. Open the terminal to inspect the worker."));
                        break;
                    }
                    let snapshot = client.read_status().unwrap_or_else(Snapshot::failed);
                    let active = snapshot.active();
                    if previous.as_ref() != Some(&snapshot) {
                        if !notify(snapshot.clone()) {
                            break;
                        }
                        previous = Some(snapshot);
                    }
                    if !active {
                        break;
                    }
                    std::thread::sleep(Duration::from_secs(1));
                }
                client.location.monitoring.store(false, Ordering::Release);
            });
        if let Err(error) = started {
            self.location.monitoring.store(false, Ordering::Release);
            return Err(format!("Couldn't monitor fork updater: {error}"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_accepts_worker_fields_but_rejects_unknown_protocol() {
        let state: Snapshot = serde_json::from_str(r#"{"schema":1,"phase":"needs_input","message":"Which behavior?","version":"0.15.0","run_id":"extra"}"#).unwrap();
        assert!(state.active());
        assert!(!state.ready());
        assert!(state.validate().is_ok());
        assert!(
            Snapshot {
                schema: 2,
                phase: "ready".into(),
                ..Snapshot::default()
            }
            .validate()
            .is_err()
        );
        assert!(
            Snapshot {
                schema: 1,
                phase: "execute_arbitrary_command".into(),
                ..Snapshot::default()
            }
            .validate()
            .is_err()
        );
    }

    #[test]
    fn missing_or_oversized_state_is_an_error_not_ready() {
        let root = std::env::temp_dir().join(format!(
            "spotifast-updater-state-{:016x}",
            rand::random::<u64>()
        ));
        std::fs::create_dir(&root).unwrap();
        let client = Client::at(root.join("not-executed"), root.clone());
        assert!(client.read_status().is_err());
        std::fs::write(root.join("state.json"), vec![b' '; 65_537]).unwrap();
        assert!(client.read_status().is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
