use ansi_to_tui::IntoText;
use ratatui::text::Text;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    mpsc::{self, Receiver, Sender},
    Arc,
};
use std::time::{Duration, Instant};
use toml_edit::DocumentMut;

pub const SCENARIOS: &[&str] = &[
    "Current directory",
    "Successful command",
    "Failed command",
    "Long-running command",
    "Background jobs",
];
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scenario {
    pub index: usize,
    pub width: u16,
    pub directory: String,
    pub status: i32,
    pub duration: u64,
    pub jobs: usize,
}
impl Default for Scenario {
    fn default() -> Self {
        Self {
            index: 0,
            width: 80,
            directory: std::env::current_dir()
                .unwrap_or_default()
                .display()
                .to_string(),
            status: 0,
            duration: 0,
            jobs: 0,
        }
    }
}
impl Scenario {
    pub fn cycle(&mut self) {
        self.index = (self.index + 1) % SCENARIOS.len();
        self.status = if self.index == 2 { 1 } else { 0 };
        self.duration = if self.index == 3 { 6500 } else { 0 };
        self.jobs = if self.index == 4 { 2 } else { 0 };
    }
}
struct Job {
    id: u64,
    config: String,
    scenario: Scenario,
    module: Option<String>,
}
struct Done {
    id: u64,
    body: Result<(String, Option<(String, bool)>), String>,
}
pub struct Previewer {
    tx: Option<Sender<Job>>,
    worker: Option<std::thread::JoinHandle<()>>,
    rx: Receiver<Done>,
    generation: Arc<AtomicU64>,
    pub result: Option<Result<String, String>>,
    pub visibility: Option<(String, bool)>,
    key: String,
    changed: Instant,
    submitted: bool,
}
impl Previewer {
    pub fn new() -> Self {
        let (tx, rx_job) = mpsc::channel::<Job>();
        let (tx_done, rx) = mpsc::channel();
        let generation = Arc::new(AtomicU64::new(0));
        let token = generation.clone();
        let worker = std::thread::spawn(move || {
            while let Ok(mut job) = rx_job.recv() {
                while let Ok(next) = rx_job.try_recv() {
                    job = next;
                }
                if token.load(Ordering::SeqCst) != job.id {
                    continue;
                }
                let body = render(&job, &token);
                let _ = tx_done.send(Done { id: job.id, body });
            }
        });
        Self {
            tx: Some(tx),
            worker: Some(worker),
            rx,
            generation,
            result: None,
            visibility: None,
            key: String::new(),
            changed: Instant::now(),
            submitted: false,
        }
    }
    pub fn request(
        &mut self,
        config: &str,
        scenario: &Scenario,
        approved: bool,
        module: Option<String>,
    ) {
        let safe = safe_config(config, approved);
        let key = format!("{scenario:?}\n{safe}");
        if key != self.key {
            self.cancel();
            self.key = key.clone();
            self.result = None;
            self.visibility = None;
            self.changed = Instant::now();
            self.submitted = false;
        }
        if !self.submitted && self.changed.elapsed() >= Duration::from_millis(180) {
            self.submitted = true;
            if let Some(tx) = &self.tx {
                let _ = tx.send(Job {
                    id: self.generation.load(Ordering::SeqCst),
                    config: safe,
                    scenario: scenario.clone(),
                    module,
                });
            }
        }
    }
    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        while let Ok(done) = self.rx.try_recv() {
            if done.id == self.generation.load(Ordering::SeqCst) {
                match done.body {
                    Ok((body, visibility)) => {
                        self.result = Some(Ok(body));
                        self.visibility = visibility;
                    }
                    Err(e) => self.result = Some(Err(e)),
                }
                changed = true;
            }
        }
        changed
    }
    pub fn cancel(&mut self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.result = Some(Err("Preview cancelled".into()));
        self.submitted = true;
    }
}
impl Drop for Previewer {
    fn drop(&mut self) {
        self.cancel();
        self.tx.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
pub fn custom_definitions(config: &str) -> String {
    config
        .parse::<DocumentMut>()
        .ok()
        .and_then(|d| d.get("custom").map(ToString::to_string))
        .unwrap_or_default()
}
pub fn safe_config(config: &str, approved: bool) -> String {
    if approved {
        return config.into();
    }
    let Ok(mut doc) = config.parse::<DocumentMut>() else {
        return String::new();
    };
    if let Some(table) = doc.get_mut("custom").and_then(|i| i.as_table_like_mut()) {
        for (_, item) in table.iter_mut() {
            if let Some(module) = item.as_table_like_mut() {
                module.insert("disabled", toml_edit::value(true));
                module.remove("when");
            }
        }
    }
    doc.to_string()
}
/// Bounded CLI calls with private output files avoid pipe deadlocks. Kill the entire process group.
pub fn bounded(mut command: Command, token: Option<(&AtomicU64, u64)>) -> Result<String, String> {
    use std::os::unix::process::CommandExt;
    let out = tempfile::tempfile().map_err(|e| e.to_string())?;
    let err = tempfile::tempfile().map_err(|e| e.to_string())?;
    command
        .stdin(Stdio::null())
        .stdout(out.try_clone().map_err(|e| e.to_string())?)
        .stderr(err.try_clone().map_err(|e| e.to_string())?);
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("Starship unavailable: {e}"))?;
    let started = Instant::now();
    let status = loop {
        let cancelled = token.is_some_and(|(t, id)| t.load(Ordering::SeqCst) != id);
        if cancelled || started.elapsed() > Duration::from_secs(3) {
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGKILL);
            }
            let _ = child.wait();
            return Err(if cancelled {
                "Preview cancelled"
            } else {
                "Starship timed out after 3 seconds"
            }
            .into());
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(Duration::from_millis(15)),
            Err(e) => return Err(e.to_string()),
        }
    };
    use std::io::{Seek, SeekFrom};
    let mut out = out;
    let mut err = err;
    let _ = out.seek(SeekFrom::Start(0));
    let _ = err.seek(SeekFrom::Start(0));
    let mut body = String::new();
    let mut diagnostic = String::new();
    out.take(1024 * 1024)
        .read_to_string(&mut body)
        .map_err(|e| e.to_string())?;
    err.take(65536)
        .read_to_string(&mut diagnostic)
        .map_err(|e| e.to_string())?;
    if !status.success() || !diagnostic.trim().is_empty() {
        return Err(format!("Starship {status}: {}", diagnostic.trim()));
    }
    Ok(body)
}
pub fn default_config() -> Result<String, String> {
    let empty = tempfile::NamedTempFile::new().map_err(|e| e.to_string())?;
    let mut c = Command::new("starship");
    c.arg("print-config").env("STARSHIP_CONFIG", empty.path());
    bounded(c, None)
}
pub fn cli(args: &[&str]) -> Result<String, String> {
    let mut c = Command::new("starship");
    c.args(args);
    bounded(c, None)
}
fn render(job: &Job, token: &AtomicU64) -> Result<(String, Option<(String, bool)>), String> {
    let mut config = tempfile::NamedTempFile::new().map_err(|e| e.to_string())?;
    config
        .write_all(job.config.as_bytes())
        .map_err(|e| e.to_string())?;
    let mut results = vec![];
    for right in [false, true] {
        let mut c = Command::new("starship");
        c.arg("prompt");
        if right {
            c.arg("--right");
        }
        c.args([
            "--terminal-width",
            &job.scenario.width.to_string(),
            "--path",
            &job.scenario.directory,
            "--status",
            &job.scenario.status.to_string(),
            "--cmd-duration",
            &job.scenario.duration.to_string(),
            "--jobs",
            &job.scenario.jobs.to_string(),
        ]);
        c.env("STARSHIP_CONFIG", config.path())
            .env("TERM", "xterm-256color");
        let text = bounded(c, Some((token, job.id)))?;
        results.push(format!(
            "{}\n{}",
            if right { "Right prompt" } else { "Left prompt" },
            clean_prompt(&text)
        ));
    }
    let visibility = if let Some(module) = &job.module {
        let mut c = Command::new("starship");
        c.args([
            "module",
            module,
            "--terminal-width",
            &job.scenario.width.to_string(),
            "--path",
            &job.scenario.directory,
            "--status",
            &job.scenario.status.to_string(),
            "--cmd-duration",
            &job.scenario.duration.to_string(),
            "--jobs",
            &job.scenario.jobs.to_string(),
        ]);
        c.env("STARSHIP_CONFIG", config.path())
            .env("TERM", "xterm-256color");
        let output = bounded(c, Some((token, job.id)))?;
        Some((module.clone(), !output.trim().is_empty()))
    } else {
        None
    };
    Ok((results.join("\n\n"), visibility))
}
pub fn render_ansi(ansi: &str) -> Text<'static> {
    let text = clean_prompt(ansi);
    text.clone().into_text().unwrap_or_else(|_| Text::raw(text))
}
fn clean_prompt(text: &str) -> String {
    text.replace("\x1b[J", "")
        .replace("\x1b[0J", "")
        .replace("\x1b[2J", "")
        .replace("\x1b[H", "")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selection_does_not_invalidate_preview() {
        let mut preview = Previewer::new();
        let scenario = Scenario::default();
        preview.request(
            "format = '$directory'",
            &scenario,
            false,
            Some("directory".into()),
        );
        preview.result = Some(Ok("current preview".into()));
        let generation = preview.generation.load(Ordering::SeqCst);
        preview.request(
            "format = '$directory'",
            &scenario,
            false,
            Some("git_branch".into()),
        );
        assert_eq!(preview.generation.load(Ordering::SeqCst), generation);
        assert_eq!(preview.result, Some(Ok("current preview".into())));
        preview.request(
            "format = '$git_branch'",
            &scenario,
            false,
            Some("git_branch".into()),
        );
        assert!(preview.result.is_none());
    }
    #[test]
    fn approval_preserves_real_draft() {
        let s = "[custom.test]\ncommand = 'echo hello'\nwhen = 'true'\n";
        let safe = safe_config(s, false);
        assert!(safe.contains("disabled = true"));
        assert!(!safe.contains("when"));
        assert_eq!(safe_config(s, true), s);
    }
    #[test]
    fn timeout_and_cancel() {
        let mut c = Command::new("sleep");
        c.arg("5");
        assert!(bounded(c, None).unwrap_err().contains("timed out"));
        let t = AtomicU64::new(2);
        let c = Command::new("sleep");
        assert!(bounded(c, Some((&t, 1))).unwrap_err().contains("cancelled"));
    }
    #[test]
    fn inline_custom_commands_are_disabled() {
        let original = "custom = { test = {command = 'do not execute', when = 'also blocked'} }";
        let safe = safe_config(original, false);
        let d = safe.parse::<DocumentMut>().unwrap();
        let t = d["custom"]
            .as_table_like()
            .unwrap()
            .get("test")
            .unwrap()
            .as_table_like()
            .unwrap();
        assert_eq!(t.get("disabled").unwrap().as_bool(), Some(true));
        assert!(t.get("when").is_none());
        assert!(original.contains("also blocked"));
    }
    #[test]
    fn cancellation_interrupts_a_running_child() {
        let token = Arc::new(AtomicU64::new(0));
        let running = token.clone();
        let started = Instant::now();
        let worker = std::thread::spawn(move || {
            let mut c = Command::new("sleep");
            c.arg("30");
            bounded(c, Some((&running, 0)))
        });
        std::thread::sleep(Duration::from_millis(60));
        token.fetch_add(1, Ordering::SeqCst);
        assert!(worker.join().unwrap().unwrap_err().contains("cancelled"));
        assert!(started.elapsed() < Duration::from_secs(1));
    }
}
