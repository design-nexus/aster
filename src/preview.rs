//! `starship prompt` against a temp config. Results are cached by the
//! config text so moving back through the wizard does not re-run it.

use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use ansi_to_tui::IntoText;
use ratatui::text::Text;

static JOBS: AtomicU64 = AtomicU64::new(0);

struct Job {
    key: String,
    config: String,
    columns: u16,
    term: String,
}

struct Done {
    key: String,
    body: Result<String, String>,
}

pub struct Previewer {
    tx: Sender<Job>,
    rx: Receiver<Done>,
    cache: HashMap<String, Result<String, String>>,
    pending: HashSet<String>,
    term: String,
}

impl Previewer {
    pub fn new() -> Self {
        let (tx, job_rx) = mpsc::channel::<Job>();
        let (done_tx, rx) = mpsc::channel::<Done>();
        thread::spawn(move || worker(job_rx, done_tx));
        let term = std::env::var("TERM").unwrap_or_default();
        let term = if term.is_empty() || term == "dumb" {
            "xterm-256color".to_string()
        } else {
            term
        };
        Self {
            tx,
            rx,
            cache: HashMap::new(),
            pending: HashSet::new(),
            term,
        }
    }

    /// Returns true when a prompt finished rendering.
    pub fn poll(&mut self) -> bool {
        let mut ready = false;
        while let Ok(done) = self.rx.try_recv() {
            ready = true;
            self.pending.remove(&done.key);
            if self.cache.len() > 48 {
                self.cache.clear();
            }
            self.cache.insert(done.key, done.body);
        }
        ready
    }

    pub fn get(&mut self, config: &str, columns: u16) -> Option<&Result<String, String>> {
        let key = cache_key(columns, config);
        if self.cache.contains_key(&key) {
            return self.cache.get(&key);
        }
        if self.pending.insert(key.clone()) {
            let _ = self.tx.send(Job {
                key,
                config: config.to_string(),
                columns,
                term: self.term.clone(),
            });
        }
        None
    }
}

pub fn render_ansi(ansi: &str) -> Text<'static> {
    let cleaned = clean_prompt(ansi);
    cleaned.into_text().unwrap_or_else(|_| Text::raw(cleaned))
}

fn cache_key(columns: u16, config: &str) -> String {
    format!("{columns}\n{config}")
}

fn worker(rx: Receiver<Job>, tx: Sender<Done>) {
    while let Ok(job) = rx.recv() {
        let body = render(&job);
        let _ = tx.send(Done { key: job.key, body });
    }
}

fn render(job: &Job) -> Result<String, String> {
    let id = JOBS.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("starship-editor-{id}.toml"));
    let mut file = std::fs::File::create(&path).map_err(|err| err.to_string())?;
    file.write_all(job.config.as_bytes())
        .map_err(|err| err.to_string())?;
    let _ = file.flush();
    drop(file);
    let output = Command::new("starship")
        .args(["prompt", "--status", "0"])
        .env("STARSHIP_CONFIG", &path)
        .env("TERM", &job.term)
        .env("COLUMNS", job.columns.max(20).to_string())
        .stdin(Stdio::null())
        .output()
        .map_err(|err| format!("starship: {err}"))?;
    let _ = std::fs::remove_file(&path);
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !output.status.success() {
        return Err(if stderr.is_empty() {
            format!("starship exited {}", output.status)
        } else {
            stderr
        });
    }
    let cleaned = clean_prompt(&stdout);
    if cleaned.contains("Starship disabled") {
        return Err(cleaned.trim().to_string());
    }
    Ok(cleaned)
}

fn clean_prompt(text: &str) -> String {
    text.replace("\u{1b}[J", "")
        .replace("\u{1b}[0J", "")
        .replace("\u{1b}[2J", "")
        .replace("\u{1b}[H", "")
}
