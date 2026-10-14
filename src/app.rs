use crate::{
    config,
    editor::{self, Action},
    preview::{self, Previewer, Scenario},
    theme::Theme,
    wizard::{Catalog, Wizard},
    workspace::Workspace,
};
use crossterm::event::{KeyCode as K, KeyEvent, KeyModifiers as M, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use toml_edit::{DocumentMut, Item};

pub const SCREENS: &[&str] = &[
    "Edit prompt",
    "All settings",
    "Presets",
    "Source",
    "Review changes",
];
pub const PANES: &[&[&str]] = &[
    &["Parts", "Settings", "Preview"],
    &["Catalog", "Properties", "Preview"],
    &["Presets", "Details", "Preview"],
    &["TOML", "Diagnostics"],
    &["Findings", "Diff", "Save"],
];
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Intent {
    Edit(Action),
    SetColor(bool, String),
    SetPalette(usize),
    Advanced,
    Scenarios,
    Screen(usize),
    Pane(usize),
    Move(isize),
    Activate,
    Toggle,
    Review,
    Save,
    SaveAs,
    Undo,
    Redo,
    New,
    Quit,
    Cancel,
    Navigation,
    Actions,
    Help,
    Filter,
    CycleFilter,
    SetFilter(usize),
    Scenario,
    SetScenario(usize),
    ScrollX(isize),
    ScenarioEdit,
    Approve,
    Theme,
    AddKey,
    RemoveKey,
    Reorder(isize),
    Placement,
    EditLeft,
    EditRight,
    AdoptPreset,
    Return,
    Select(usize),
    FocusSelect(usize, usize),
    Scroll(isize),
    ModalAccept,
    ModalCancel,
    ModalApprove,
    ModalDiscard,
    ModalReview,
    BuilderMove(isize),
    BuilderReorder(isize),
    BuilderNext,
    BuilderBack,
    BuilderCancel,
    BuilderToggle,
    BuilderColor(isize),
    BuilderHex,
    BuilderText,
    BuilderPreview,
    BuilderSelect(usize),
}
#[derive(Clone)]
pub struct Hit {
    pub area: Rect,
    pub intent: Intent,
}
pub enum Overlay {
    Picker {
        title: String,
        help: String,
        options: Vec<(String, Intent)>,
        selected: usize,
        query: String,
    },
    Menu {
        navigation: bool,
        query: String,
        selected: usize,
    },
    Help,
    Quit,
    Approval,
    Preset(String),
    Wizard,
    Input(Input),
}
#[derive(Clone)]
pub enum InputTarget {
    Plain(Vec<String>),
    Number(Vec<String>),
    Color(bool),
    Property(Vec<String>),
    SaveAs,
    Search,
    Scenario,
    Left,
    Right,
}
pub struct Input {
    pub title: String,
    pub text: String,
    pub cursor: usize,
    pub target: InputTarget,
    pub error: Option<String>,
    pub help: String,
}
#[derive(Clone, Debug)]
pub struct Segment {
    pub text: String,
    pub module: Option<String>,
    pub right: bool,
}
pub struct App {
    pub workspace: Workspace,
    pub theme: Theme,
    pub screen: usize,
    pub setting_group: usize,
    pub prompt_settings: bool,
    pub editor_body: Option<Rect>,
    pub panes: [usize; 5],
    pub selections: [[usize; 3]; 5],
    pub scrolls: [[usize; 3]; 5],
    pub horizontal: [[usize; 3]; 5],
    pub overlay: Option<Overlay>,
    pub wizard: Option<Wizard>,
    pub preview: Previewer,
    pub scenario: Scenario,
    pub notice: String,
    pub quit: bool,
    pub filter: String,
    pub module_filter: usize,
    pub modules: Vec<String>,
    pub defaults: DocumentMut,
    pub presets: Vec<String>,
    pub preset_text: Option<(String, String)>,
    pub hits: Vec<Hit>,
    pub raw_cursor: usize,
    pub raw_editing: bool,
    pub approved: String,
    pub appearance: usize,
    theme_tick: Instant,
    pub last_click: Option<(usize, usize, usize, Instant)>,
    pub overlay_scroll: usize,
    pub wizard_preview: bool,
    preset_tx: Option<std::sync::mpsc::Sender<(String, u64)>>,
    preset_worker: Option<std::thread::JoinHandle<()>>,
    preset_token: std::sync::Arc<std::sync::atomic::AtomicU64>,
    preset_cancelled: bool,
    preset_rx: std::sync::mpsc::Receiver<(String, u64, Result<String, String>)>,
    preset_pending: Option<String>,
}
impl App {
    pub fn new(path: PathBuf, edit: bool, wizard: bool) -> Self {
        let catalog = Catalog::load();
        let defaults = preview::default_config()
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or_default();
        let (mut presets, preset_error) = match preview::cli(&["preset", "--list"]) {
            Ok(s) => (
                s.lines()
                    .map(str::trim)
                    .filter(|s| {
                        !s.is_empty() && !s.starts_with("Available") && !s.starts_with("Presets")
                    })
                    .map(str::to_string)
                    .collect(),
                String::new(),
            ),
            Err(e) => (vec![], e),
        };
        let mut bundled: Vec<String> = crate::presets::BUNDLED
            .iter()
            .map(|(name, _)| (*name).to_string())
            .collect();
        bundled.append(&mut presets);
        let presets = bundled;
        let pref = preference_path();
        let appearance = std::fs::read_to_string(pref)
            .ok()
            .and_then(|s| s.trim().parse::<usize>().ok())
            .unwrap_or(0)
            .min(2);
        let (preset_tx, requests) = std::sync::mpsc::channel::<(String, u64)>();
        let (results, preset_rx) = std::sync::mpsc::channel();
        let preset_token = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let token = preset_token.clone();
        let preset_worker = std::thread::spawn(move || {
            while let Ok(mut job) = requests.recv() {
                while let Ok(next) = requests.try_recv() {
                    job = next;
                }
                let (name, id) = job;
                let text = if let Some(text) = crate::presets::get(&name) {
                    Ok(text.to_string())
                } else {
                    let mut command = std::process::Command::new("starship");
                    command.args(["preset", &name]);
                    preview::bounded(command, Some((&token, id)))
                };
                if results.send((name, id, text)).is_err() {
                    break;
                }
            }
        });
        let mut app = Self {
            workspace: Workspace::open(path),
            theme: Theme::aurora(),
            screen: usize::from(edit),
            setting_group: 0,
            prompt_settings: false,
            editor_body: None,
            panes: [0; 5],
            selections: [[0; 3]; 5],
            scrolls: [[0; 3]; 5],
            horizontal: [[0; 3]; 5],
            overlay: None,
            wizard: None,
            preview: Previewer::new(),
            scenario: Scenario::default(),
            notice: catalog.starship_error.unwrap_or(preset_error),
            quit: false,
            filter: String::new(),
            module_filter: 0,
            modules: catalog.modules,
            defaults,
            presets,
            preset_text: None,
            hits: vec![],
            raw_cursor: 0,
            raw_editing: false,
            approved: String::new(),
            appearance,
            theme_tick: Instant::now(),
            last_click: None,
            overlay_scroll: 0,
            wizard_preview: false,
            preset_tx: Some(preset_tx),
            preset_worker: Some(preset_worker),
            preset_token,
            preset_cancelled: false,
            preset_rx,
            preset_pending: None,
        };
        app.refresh_theme();
        if app.workspace.error.is_some() || app.workspace.load_error.is_some() {
            app.screen = 3;
        }
        if wizard && app.workspace.error.is_none() {
            app.dispatch(Intent::New);
        }
        app
    }
    pub fn pane(&self) -> usize {
        self.panes[self.screen]
    }
    pub fn selected(&self) -> usize {
        self.selections[self.screen][self.pane()]
    }
    pub fn set_selected(&mut self, n: usize) {
        let p = self.pane();
        self.selections[self.screen][p] = n;
    }
    pub fn tick(&mut self) {
        while let Ok((name, id, result)) = self.preset_rx.try_recv() {
            if self.preset_pending.as_ref() == Some(&name)
                && id == self.preset_token.load(std::sync::atomic::Ordering::SeqCst)
            {
                self.preset_pending = None;
                match result {
                    Ok(text) => self.preset_text = Some((name, text)),
                    Err(e) => {
                        self.notice = e;
                        self.preset_text = None;
                        self.preset_cancelled = true;
                    }
                }
            }
        }
        if self.screen == 2
            && self.preset_text.is_none()
            && self.preset_pending.is_none()
            && !self.preset_cancelled
        {
            self.load_preset();
        }
        if self.theme_tick.elapsed() > Duration::from_secs(1) {
            self.refresh_theme();
            self.theme_tick = Instant::now();
        }
        let text = if matches!(self.overlay, Some(Overlay::Wizard)) {
            self.wizard
                .as_mut()
                .map(Wizard::preview)
                .unwrap_or_default()
        } else if self.screen == 2 {
            self.preset_text
                .as_ref()
                .map(|(_, s)| s.clone())
                .unwrap_or_else(|| self.workspace.doc.to_string())
        } else {
            self.workspace.doc.to_string()
        };
        let approved =
            !self.approved.is_empty() && self.approved == preview::custom_definitions(&text);
        let module = if self.screen == 0 || self.screen == 1 {
            self.module()
                .filter(|m| !["all", "line_break", "fill"].contains(&m.as_str()))
        } else {
            None
        };
        self.preview
            .request(&text, &self.scenario, approved, module);
        self.preview.poll();
    }
    fn refresh_theme(&mut self) {
        self.theme = match self.appearance {
            1 => crate::theme::load_omarchy().unwrap_or_else(Theme::aurora),
            2 => Theme::terminal(),
            _ => Theme::aurora(),
        };
    }
    pub fn part_status(&self, module: &str) -> String {
        if module == "all" {
            return "Automatic Starship content".into();
        }
        if !self.enabled(module) {
            return "Included in prompt · Turned off".into();
        }
        if ["line_break", "fill"].contains(&module) {
            return "Included in prompt · Layout element".into();
        }
        match self
            .preview
            .visibility
            .as_ref()
            .filter(|(m, _)| m == module)
        {
            Some((_, true)) => "Included in prompt · Visible in this preview".into(),
            Some((_, false)) => "Hidden in this preview — Starship produced no output".into(),
            None => "Included in prompt · Checking preview visibility…".into(),
        }
    }
    pub fn segments(&self) -> Vec<Segment> {
        let mut out = vec![];
        for right in [false, true] {
            let text = if right {
                self.workspace
                    .doc
                    .get("right_format")
                    .and_then(Item::as_str)
                    .unwrap_or("")
                    .to_string()
            } else {
                config::logical_format(&self.workspace.doc)
            };
            for module in editor::variables(&text) {
                out.push(Segment {
                    text: format!("${module}"),
                    module: Some(module),
                    right,
                });
            }
            if !text.is_empty() && editor::variables(&text).is_empty() {
                out.push(Segment {
                    text,
                    module: None,
                    right,
                });
            }
        }
        out
    }
    pub fn catalog(&self) -> Vec<String> {
        let mut names = self.modules.clone();
        collect_modules(&self.workspace.doc, &mut names);
        collect_modules(&self.defaults, &mut names);
        names.retain(|s| s != "custom" && s != "palettes" && s != "profiles");
        names.sort();
        names.dedup();
        names
            .into_iter()
            .filter(|s| {
                s.to_lowercase().contains(&self.filter.to_lowercase())
                    && match self.module_filter {
                        1 => lookup(
                            &self.workspace.doc,
                            &s.split('.').map(str::to_string).collect::<Vec<_>>(),
                        )
                        .is_some(),
                        2 => self.included(s) && self.enabled(s),
                        _ => true,
                    }
            })
            .collect()
    }
    pub fn enabled(&self, name: &str) -> bool {
        let keys = name
            .split('.')
            .map(str::to_string)
            .chain(std::iter::once("disabled".into()))
            .collect::<Vec<_>>();
        lookup(&self.workspace.doc, &keys)
            .or_else(|| lookup(&self.defaults, &keys))
            .and_then(Item::as_bool)
            != Some(true)
    }
    pub fn included(&self, name: &str) -> bool {
        let left = config::logical_format(&self.workspace.doc);
        let right = self
            .workspace
            .doc
            .get("right_format")
            .and_then(Item::as_str)
            .unwrap_or("");
        let referenced = |format: &str, key: &str| !config::find_vars(format, key).is_empty();
        referenced(&left, name)
            || referenced(right, name)
            || ((name.starts_with("custom.") || name.starts_with("env_var."))
                && (referenced(&left, name.split('.').next().unwrap())
                    || referenced(right, name.split('.').next().unwrap())))
            || ((referenced(&left, "all") || referenced(right, "all"))
                && (self.modules.iter().any(|m| m == name)
                    || name.starts_with("custom.")
                    || name.starts_with("env_var.")))
    }
    pub fn module(&self) -> Option<String> {
        if self.screen == 0 {
            if self.prompt_settings {
                return None;
            }
            self.segments()
                .get(self.selections[0][0])
                .and_then(|s| s.module.clone())
        } else {
            self.catalog().get(self.selections[1][0]).cloned()
        }
    }
    pub fn properties(&self) -> Vec<(Vec<String>, String, String, bool)> {
        let prefix = self
            .module()
            .map(|s| s.split('.').map(str::to_string).collect::<Vec<_>>())
            .unwrap_or_default();
        let mut rows = vec![];
        flatten_properties(&self.defaults, &prefix, &mut rows, false);
        flatten_properties(&self.workspace.doc, &prefix, &mut rows, true);
        if prefix.is_empty() {
            for key in config::PROMPT_FIELDS {
                if !rows
                    .iter()
                    .any(|(p, _, _, _)| p.last().is_some_and(|s| s == key))
                {
                    rows.push((
                        vec![key.to_string()],
                        key.to_string(),
                        "default".into(),
                        false,
                    ));
                }
            }
        } else if lookup(&self.defaults, &prefix).is_none() {
            for key in config::MODULE_FIELDS {
                if !rows
                    .iter()
                    .any(|(p, _, _, _)| p.last().is_some_and(|s| s == key))
                {
                    let mut p = prefix.clone();
                    p.push(key.to_string());
                    rows.push((p, key.to_string(), "default".into(), false));
                }
            }
        }
        rows.sort_by(|a, b| a.0.cmp(&b.0));
        rows
    }
    pub fn rows_for_parts(&self) -> Vec<String> {
        self.segments()
            .iter()
            .map(|s| {
                let name = s
                    .module
                    .as_deref()
                    .map(editor::name)
                    .unwrap_or_else(|| "Custom text".into());
                format!(
                    "{name}{}{}",
                    if s.right { " →" } else { "" },
                    if s.module.as_ref().is_some_and(|m| !self.enabled(m)) {
                        " (off)"
                    } else {
                        ""
                    }
                )
            })
            .collect()
    }
    pub fn rows(&self) -> Vec<String> {
        match (self.screen, self.pane()) {
            (0, 0) => {
                self.segments()
                    .into_iter()
                    .map(|s| {
                        let name = s.module.as_deref().map(editor::name).unwrap_or_else(|| {
                            format!("Custom text: {}", s.text.replace('\n', " ↵ "))
                        });
                        let state = if s.module.as_ref().is_some_and(|m| !self.enabled(m)) {
                            " · Off"
                        } else {
                            ""
                        };
                        format!("{name} · {}{state}", if s.right { "Right" } else { "Left" })
                    })
                    .collect()
            }
            (0, 1) => editor::controls(self)
                .iter()
                .map(|c| c.label.clone())
                .collect(),
            (1, 1) => self
                .properties()
                .iter()
                .map(|(_, k, v, explicit)| {
                    format!("{k} = {v}{}", if *explicit { "" } else { "  (default)" })
                })
                .collect(),
            (1, 0) => self
                .catalog()
                .into_iter()
                .map(|s| {
                    format!(
                        "{} {s} · {}",
                        if self.enabled(&s) { "✓" } else { "○" },
                        if self.included(&s) {
                            "Included in prompt"
                        } else {
                            "Not in prompt"
                        }
                    )
                })
                .collect(),
            (2, 0) => self.presets.clone(),
            (4, 2) => vec![
                "Save to target".into(),
                "Save as…".into(),
                "Return to Edit prompt".into(),
            ],
            _ => vec![],
        }
    }
    pub fn menu(&self, navigation: bool, query: &str) -> Vec<(String, Intent)> {
        let mut items = if navigation {
            let mut v = SCREENS
                .iter()
                .enumerate()
                .map(|(i, s)| (format!("{} {s}", i + 1), Intent::Screen(i)))
                .collect::<Vec<_>>();
            v.extend(
                PANES[self.screen]
                    .iter()
                    .enumerate()
                    .map(|(i, s)| (format!("Pane: {s}"), Intent::Pane(i))),
            );
            v.extend(
                preview::SCENARIOS
                    .iter()
                    .enumerate()
                    .map(|(i, s)| (format!("Scenario: {s}"), Intent::SetScenario(i))),
            );
            if self.screen == 1 {
                v.extend(
                    ["All", "Configured", "Active"]
                        .iter()
                        .enumerate()
                        .map(|(i, s)| (format!("Filter: {s}"), Intent::SetFilter(i))),
                );
            }
            v
        } else {
            vec![
                ("Add prompt part", Intent::Edit(Action::Add)),
                ("Create new guided prompt", Intent::New),
                ("Save", Intent::Save),
                ("Review and save", Intent::Review),
                ("Save as…", Intent::SaveAs),
                ("Undo", Intent::Undo),
                ("Redo", Intent::Redo),
                ("Edit left format", Intent::EditLeft),
                ("Edit right format", Intent::EditRight),
                ("Add property (path = TOML value)", Intent::AddKey),
                ("Remove selected property", Intent::RemoveKey),
                ("Move piece earlier", Intent::Reorder(-1)),
                ("Move piece later", Intent::Reorder(1)),
                ("Move piece left / right", Intent::Placement),
                ("Next preview scenario", Intent::Scenario),
                ("Edit scenario values", Intent::ScenarioEdit),
                ("Approve custom preview commands", Intent::Approve),
                ("Change editor appearance", Intent::Theme),
                ("Adopt selected preset", Intent::AdoptPreset),
                ("Cancel preview", Intent::Cancel),
                ("Help", Intent::Help),
                ("Quit", Intent::Quit),
            ]
            .into_iter()
            .map(|(s, i)| (s.to_string(), i))
            .collect()
        };
        if !navigation {
            for (label, intent) in &mut items {
                if let Some(reason) = self.unavailable(intent) {
                    label.push_str(&format!(" — unavailable: {reason}"));
                }
            }
        }
        items.retain(|(s, _)| s.to_lowercase().contains(&query.to_lowercase()));
        items
    }
    fn unavailable(&self, intent: &Intent) -> Option<&'static str> {
        match intent {
            Intent::Edit(action)
                if !matches!(action, Action::Group(_) | Action::Advanced)
                    && (self.workspace.error.is_some() || self.workspace.load_error.is_some()) =>
            {
                Some("open Advanced → Source to correct the configuration first")
            }
            Intent::SetColor(_, _) | Intent::SetPalette(_)
                if self.workspace.error.is_some() || self.workspace.load_error.is_some() =>
            {
                Some("correct the source first")
            }
            Intent::Undo if !self.workspace.can_undo() => Some("no earlier draft"),
            Intent::Redo if !self.workspace.can_redo() => Some("no later draft"),
            Intent::New
            | Intent::AddKey
            | Intent::EditLeft
            | Intent::EditRight
            | Intent::Undo
            | Intent::Redo
                if self.workspace.error.is_some() || self.workspace.load_error.is_some() =>
            {
                Some("correct the source in TOML first")
            }
            Intent::RemoveKey
                if ![0, 1].contains(&self.screen) || self.pane() != 1 || self.screen == 0 =>
            {
                Some("focus a selected property")
            }
            Intent::Reorder(_) | Intent::Placement if self.screen != 0 || self.pane() != 0 => {
                Some("focus a composition piece")
            }
            Intent::Toggle if ![0, 1].contains(&self.screen) => {
                Some("select a part in Edit prompt or All settings")
            }
            Intent::AdoptPreset if self.screen != 2 => Some("select a preset on Presets"),
            _ => None,
        }
    }
    pub fn dispatch(&mut self, intent: Intent) {
        if let Some(reason) = self.unavailable(&intent) {
            self.notice = format!("Unavailable: {reason}");
            return;
        }
        match intent {
            Intent::Edit(action) => self.editor_action(action),
            Intent::SetColor(bg, color) => self.editor_action(Action::SetColor(bg, color)),
            Intent::SetPalette(n) => self.editor_action(Action::SetPalette(n)),
            Intent::Scenarios=>self.picker("Preview scenario","Preview a successful, failed, long-running, or background command without running those commands.",preview::SCENARIOS.iter().enumerate().map(|(n,s)|(s.to_string(),Intent::SetScenario(n))).collect()),
            Intent::Advanced => self.picker(
                "Advanced",
                "Full technical controls are here. Use Edit prompt for everyday changes.",
                vec![
                    (
                        "All module settings — detection rules and technical properties".into(),
                        Intent::Screen(1),
                    ),
                    (
                        "Source — edit the TOML configuration".into(),
                        Intent::Screen(3),
                    ),
                    (
                        "Review changes — compare saved and unsaved versions".into(),
                        Intent::Review,
                    ),
                    (
                        "Editor appearance — change Aster’s own colors".into(),
                        Intent::Theme,
                    ),
                    (
                        "Edit left format — advanced Starship expression".into(),
                        Intent::EditLeft,
                    ),
                    (
                        "Edit right format — advanced Starship expression".into(),
                        Intent::EditRight,
                    ),
                ],
            ),
            Intent::ModalAccept
            | Intent::ModalCancel
            | Intent::ModalApprove
            | Intent::ModalDiscard
            | Intent::ModalReview
            | Intent::BuilderMove(_)
            | Intent::BuilderReorder(_)
            | Intent::BuilderNext
            | Intent::BuilderBack
            | Intent::BuilderCancel
            | Intent::BuilderToggle
            | Intent::BuilderColor(_)
            | Intent::BuilderHex
            | Intent::BuilderText
            | Intent::BuilderPreview
            | Intent::BuilderSelect(_) => self.modal_action(intent),
            Intent::Screen(s) => {
                self.screen = s.min(4);
                self.raw_editing = false;
            }
            Intent::Pane(p) => {
                self.panes[self.screen] = p.min(PANES[self.screen].len() - 1);
                self.raw_editing = false;
            }
            Intent::Move(delta) if self.rows().is_empty() => self.dispatch(Intent::Scroll(delta)),
            Intent::Move(delta) => {
                let len = self.rows().len();
                let n = self
                    .selected()
                    .saturating_add_signed(delta)
                    .min(len.saturating_sub(1));
                self.set_selected(n);
                if self.screen==0 && self.pane()==0{self.prompt_settings=false;self.selections[0][1]=0;}
                if self.screen == 2 && self.pane() == 0 {
                    self.load_preset();
                }
            }
            Intent::FocusSelect(p, n) => {
                self.panes[self.screen] = p;
                self.set_selected(n);
                if self.screen == 0 && self.pane() == 0 {
                    self.prompt_settings=false;
                    self.selections[0][1] = 0;
                }
                if self.screen == 2 {
                    self.load_preset();
                }
            }
            Intent::Select(n) => {
                self.set_selected(n);
                if self.screen == 0 && self.pane() == 0 {
                    self.prompt_settings=false;
                    self.selections[0][1] = 0;
                }
                if self.screen == 2 {
                    self.load_preset();
                }
            }
            Intent::Scroll(d) if self.overlay.is_some() => {
                self.overlay_scroll = self.overlay_scroll.saturating_add_signed(d)
            }
            Intent::Scroll(d) => {
                let p = self.pane();
                self.scrolls[self.screen][p] =
                    self.scrolls[self.screen][p].saturating_add_signed(d);
            }
            Intent::Review => {
                self.screen = 4;
                self.raw_editing = false;
                let _ = self.workspace.apply_raw();
            }
            Intent::Save => {
                self.notice = match self.workspace.save(None) {
                    Ok(()) => format!(
                        "Saved to {} · backup available in Advanced → Review changes",
                        self.workspace.path.display()
                    ),
                    Err(e) => format!("Save failed: {e:#}"),
                };
            }
            Intent::SaveAs => self.input(
                "Save as — new target path",
                String::new(),
                InputTarget::SaveAs,
            ),
            Intent::Undo => {
                if self.workspace.error.is_some() {
                    self.notice = "Correct or apply the TOML draft before undo".into();
                } else {
                    self.workspace.undo(false)
                }
            }
            Intent::Redo => self.workspace.undo(true),
            Intent::New => {
                if self.workspace.error.is_some() || self.workspace.load_error.is_some() {
                    self.notice = "Fix the source in TOML before starting a builder".into();
                } else {
                    let mut wizard = Wizard::new(
                        self.workspace.doc.to_string(),
                        crate::wizard::default_swatches(),
                    );
                    wizard.defaults = self.defaults.clone();
                    self.wizard = Some(wizard);
                    self.overlay = Some(Overlay::Wizard);
                }
            }
            Intent::Quit => {
                if self.workspace.dirty() || self.workspace.error.is_some() || self.wizard.is_some()
                {
                    self.overlay = Some(Overlay::Quit);
                } else {
                    self.quit = true;
                }
            }
            Intent::Cancel => {
                self.preview.cancel();
                self.preset_token
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                self.preset_pending = None;
                self.preset_cancelled = true;
                self.notice = "Preview / preset rendering cancelled".into();
            }
            Intent::Navigation => {
                self.overlay = Some(Overlay::Menu {
                    navigation: true,
                    query: String::new(),
                    selected: 0,
                })
            }
            Intent::Actions => {
                self.overlay = Some(Overlay::Menu {
                    navigation: false,
                    query: String::new(),
                    selected: 0,
                })
            }
            Intent::Help => self.overlay = Some(Overlay::Help),
            Intent::Filter => {
                self.input("Search modules", self.filter.clone(), InputTarget::Search)
            }
            Intent::SetFilter(n) => {
                self.module_filter = n.min(2);
                self.selections[1][0] = 0;
            }
            Intent::CycleFilter => {
                self.module_filter = (self.module_filter + 1) % 3;
                self.selections[1][0] = 0;
            }
            Intent::Scenario => self.scenario.cycle(),
            Intent::SetScenario(n) => {
                while self.scenario.index != n.min(4) {
                    self.scenario.cycle();
                }
            }
            Intent::ScrollX(d) => {
                let p = self.pane();
                self.horizontal[self.screen][p] =
                    self.horizontal[self.screen][p].saturating_add_signed(d);
            }
            Intent::ScenarioEdit => self.input(
                "Scenario: directory | width | status | duration ms | jobs",
                format!(
                    "{} | {} | {} | {} | {}",
                    self.scenario.directory,
                    self.scenario.width,
                    self.scenario.status,
                    self.scenario.duration,
                    self.scenario.jobs
                ),
                InputTarget::Scenario,
            ),
            Intent::Approve => {
                let defs = preview::custom_definitions(&self.workspace.doc.to_string());
                if defs.is_empty() {
                    self.notice = "No custom command modules configured".into();
                } else {
                    self.overlay = Some(Overlay::Approval);
                }
            }
            Intent::Theme => {
                self.appearance = (self.appearance + 1) % 3;
                self.refresh_theme();
                let p = preference_path();
                if let Some(parent) = p.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                self.notice = match std::fs::write(p, self.appearance.to_string()) {
                    Ok(()) => format!("Editor appearance: {}", self.theme.name),
                    Err(e) => format!("Appearance changed; could not store preference: {e}"),
                };
            }
            Intent::EditLeft => self.input(
                "Left format — Starship expression",
                config::logical_format(&self.workspace.doc),
                InputTarget::Left,
            ),
            Intent::EditRight => self.input(
                "Right format — Starship expression",
                self.workspace
                    .doc
                    .get("right_format")
                    .and_then(Item::as_str)
                    .unwrap_or("")
                    .into(),
                InputTarget::Right,
            ),
            Intent::AddKey => self.input(
                "Property path = TOML value (e.g. rust.detect_extensions = ['rs'])",
                String::new(),
                InputTarget::Property(vec![]),
            ),
            Intent::RemoveKey => {
                if self.pane() == 1 {
                    let selected = if self.screen == 0 {
                        self.selected().checked_sub(5)
                    } else {
                        Some(self.selected())
                    };
                    if let Some((path, _, _, _)) =
                        selected.and_then(|n| self.properties().get(n).cloned())
                    {
                        let path = path.clone();
                        self.workspace.edit(|d| remove(d, &path));
                    }
                } else {
                    self.notice = "Focus a property to remove its key".into();
                }
            }
            Intent::Reorder(delta) => self.move_segment(delta, false),
            Intent::Placement => self.move_segment(0, true),
            Intent::Return => self.screen = 0,
            Intent::AdoptPreset => {
                self.load_preset();
                if let Some((name, text)) = &self.preset_text {
                    self.overlay = Some(Overlay::Preset(format!("{name}\n{text}")));
                } else {
                    self.notice =
                        "Preset is loading or unavailable; review when it finishes".into();
                }
            }
            Intent::Toggle => {
                if self.screen == 0 && self.pane() == 1 {
                    self.dispatch(Intent::Activate);
                } else if let Some(name) = self.module().filter(|m| m != "all") {
                    let keys = editor::path(&name, "disabled");
                    let disabled = self.enabled(&name);
                    self.workspace
                        .edit(|d| set(d, &keys, toml_edit::value(disabled)));
                }
            }
            Intent::Activate => match (self.screen, self.pane()) {
                (0, 0) => {
                    self.panes[0] = 1;
                    self.selections[0][1] = 0;
                }
                (0, 1) => {
                    if let Some(control) = editor::controls(self).get(self.selected()).cloned() {
                        self.dispatch(Intent::Edit(control.action));
                    }
                }
                (1, 1) => {
                    if let Some((p, k, _, _)) = self.properties().get(self.selected()) {
                        let p = p.clone();
                        let text = lookup(&self.workspace.doc, &p)
                            .or_else(|| lookup(&self.defaults, &p))
                            .map(ToString::to_string)
                            .unwrap_or_else(|| "\"\"".into());
                        self.input(
                            &format!("{k} — advanced TOML value"),
                            text,
                            InputTarget::Property(p),
                        );
                    }
                }
                (1, 0) => self.panes[1] = 1,
                (2, 0) => self.dispatch(Intent::AdoptPreset),
                (3, 0) => {
                    self.raw_editing = true;
                    self.raw_cursor = self.raw_cursor.min(self.workspace.raw.len());
                    while !self.workspace.raw.is_char_boundary(self.raw_cursor) {
                        self.raw_cursor -= 1;
                    }
                }
                (4, 2) => self.dispatch(match self.selected() {
                    0 => Intent::Save,
                    1 => Intent::SaveAs,
                    _ => Intent::Return,
                }),
                _ => {}
            },
        }
    }
    pub fn picker_items(&self) -> Vec<(String, Intent)> {
        if let Some(Overlay::Picker { options, query, .. }) = &self.overlay {
            options
                .iter()
                .filter(|(s, _)| s.to_lowercase().contains(&query.to_lowercase()))
                .cloned()
                .collect()
        } else {
            vec![]
        }
    }
    fn picker(&mut self, title: &str, help: &str, options: Vec<(String, Intent)>) {
        self.overlay = Some(Overlay::Picker {
            title: title.into(),
            help: help.into(),
            options,
            selected: 0,
            query: String::new(),
        });
    }
    fn addable_modules(&self) -> Vec<String> {
        let mut modules = self.modules.clone();
        collect_modules(&self.workspace.doc, &mut modules);
        modules.retain(|m| {
            m != "custom"
                && (self.modules.contains(m)
                    || config::ALL_MODULES.contains(&m.as_str())
                    || m.starts_with("custom.")
                    || m.starts_with("env_var."))
        });
        modules.sort_by_key(|m| {
            config::ALL_MODULES
                .iter()
                .position(|name| *name == m)
                .unwrap_or(usize::MAX)
        });
        let mut seen = std::collections::HashSet::new();
        modules.retain(|m| seen.insert(m.clone()));
        modules
    }
    fn insert_parts(&mut self, modules: Vec<String>, label: &str) {
        if modules.is_empty() {
            self.notice = "No parts are available in this group.".into();
            return;
        }
        let missing = modules
            .iter()
            .filter(|m| !self.included(m))
            .cloned()
            .collect::<Vec<_>>();
        let mut format = config::logical_format(&self.workspace.doc);
        if !missing.is_empty() {
            let insert = editor::units(&format)
                .ok()
                .and_then(|units| units.into_iter().find(|unit| unit.modules == ["character"]))
                .map(|unit| unit.start)
                .unwrap_or(format.len());
            let text = missing
                .iter()
                .map(|m| {
                    if m.is_ascii() {
                        format!("${m}")
                    } else {
                        format!("${{{m}}}")
                    }
                })
                .collect::<String>();
            format.insert_str(insert, &text);
        }
        self.workspace.edit(|d| {
            if !missing.is_empty() {
                d["format"] = toml_edit::value(format);
            }
            for module in &modules {
                set(
                    d,
                    &editor::path(module, "disabled"),
                    toml_edit::value(false),
                );
            }
        });
        self.screen = 0;
        self.prompt_settings = false;
        self.panes[0] = 1;
        self.selections[0][0] = self
            .segments()
            .iter()
            .position(|s| s.module.as_ref().is_some_and(|m| modules.contains(m)))
            .unwrap_or(0);
        self.selections[0][1] = 0;
        self.notice =
            format!("Enabled {label}. Parts appear when detected; Undo reverses this change.");
    }
    fn editor_action(&mut self, action: Action) {
        match action {
            Action::WholePrompt=>{self.prompt_settings=true;self.panes[0]=1;self.selections[0][1]=0;},
            Action::Group(n)=>{self.setting_group=n.min(3);self.panes[0]=1;self.selections[0][1]=0;},
            Action::Builder=>self.dispatch(Intent::New),
            Action::Shift(d)=>{self.move_segment(d,false);},
            Action::Side=>{self.move_segment(0,true);},
            Action::Advanced=>{
                let module=self.module();self.screen=1;self.module_filter=0;self.filter.clear();
                if let Some(m)=module {if let Some(i)=self.catalog().iter().position(|n|n==&m){self.selections[1][0]=i;}}
                self.panes[1]=1;
            },
            Action::Add=>{
                let modules=self.addable_modules();
                let mut items=vec![("Enable all · Add and enable every part".into(),Intent::Edit(Action::EnableAll))];
                for(n,group)in editor::FAMILIES.iter().enumerate(){
                    if modules.iter().any(|m|editor::family(m)==Some(n)&&(!self.included(m)||!self.enabled(m))){items.push((format!("{} · {}",group.name,group.detail),Intent::Edit(Action::InsertFamily(n))));}
                }
                items.extend(modules.into_iter().filter(|m|editor::family(m).is_none()&&(!self.included(m)||!self.enabled(m))).map(|m|(editor::part_label(&m),Intent::Edit(Action::Insert(m)))));
                self.picker("Add prompt parts","Choose a group or a single part. Only detected content appears in the prompt.",items);
            },
            Action::Insert(m)=>{let label=editor::name(&m);self.insert_parts(vec![m],&label);},
            Action::InsertFamily(n)=>{
                if let Some(group)=editor::FAMILIES.get(n){let modules=self.addable_modules().into_iter().filter(|m|editor::family(m)==Some(n)).collect();self.insert_parts(modules,group.name);}
            },
            Action::EnableAll=>{let modules=self.addable_modules();self.insert_parts(modules,"all parts");},
            Action::Expand=>{
                let installed=self.defaults.to_string();
                let replacement=installed.lines().find_map(|line|line.strip_prefix("# $all is shorthand for ")).map(str::to_string).unwrap_or_else(||config::ALL_MODULES.iter().map(|m|format!("${m}")).collect());
                self.workspace.edit(|d|{for key in ["format","right_format"] {let text=if key=="format"{config::logical_format(d)}else{d.get(key).and_then(Item::as_str).unwrap_or("").into()};let hits=config::find_vars(&text,"all");if !hits.is_empty(){let mut text=text;for(start,end)in hits.into_iter().rev(){text.replace_range(start..end,&replacement);}d[key]=toml_edit::value(text);}}});
                self.notice="Automatic content expanded. Select a part to edit it, turn it off, or move it.".into();self.selections[0][0]=0;
            },
            Action::Remove=>{self.structural_edit(None);},
            Action::Toggle(keys)=>{
                let old=editor::value(self,&keys).and_then(|i|i.as_bool()).unwrap_or(keys==["add_newline"]);
                self.workspace.edit(|d|set(d,&keys,toml_edit::value(!old)));
            },
            Action::Text(keys)|Action::Number(keys)=>{
                let v=editor::value(self,&keys).unwrap_or_else(||toml_edit::value(""));
                let number=v.as_integer().is_some();let text=if number{v.to_string().trim().into()}else{let s=v.as_str().unwrap_or("");if keys.last().is_some_and(|s|s.ends_with("symbol")){editor::plain_symbol(s)}else{s.into()}};
                let control=editor::controls(self).into_iter().find(|c| matches!(&c.action,Action::Text(p)|Action::Number(p) if p==&keys));
                let help=control.as_ref().map(|c|c.help.clone()).unwrap_or_default();
                let title=control.map(|c|c.label.split(':').next().unwrap_or(&c.label).to_string()).unwrap_or_else(||"Edit value".into());
                self.input(&title,text,if number{InputTarget::Number(keys)}else{InputTarget::Plain(keys)});
                if let Some(Overlay::Input(input))=&mut self.overlay{input.help=help;}
            },
            Action::Color(bg)=>{
                let mut options=vec![("Default — let the terminal choose".into(),Intent::SetColor(bg,"default".into()))];
                for color in ["black","red","green","yellow","blue","purple","cyan","white","bright-black","bright-red","bright-green","bright-yellow","bright-blue","bright-purple","bright-cyan","bright-white"] {options.push((color.replace('-'," "),Intent::SetColor(bg,color.into())));}
                options.push(("Custom hex color…".into(),Intent::Edit(Action::SetColor(bg,"custom".into()))));
                self.picker(if bg{"Background color"}else{"Text color"},"ANSI colors follow your terminal palette. Default inherits the surrounding style. Colors in shared groups can also affect decorations.",options);
            },
            Action::SetColor(bg,color)=>{
                if color=="custom"{self.input("Custom color — #RRGGBB","#".into(),InputTarget::Color(bg));return;}
                if let Some(module)=self.module().filter(|m|m!="all") {
                    if module=="character" {
                        let values=["success_symbol","error_symbol","vimcmd_symbol"].iter().filter(|k|bg||**k!="error_symbol").map(|k|{let keys=editor::path(&module,k);let old=editor::value(self,&keys).and_then(|i|i.as_str().map(str::to_string)).unwrap_or_default();(keys,editor::color_symbol(&old,bg,&color))}).collect::<Vec<_>>();
                        self.workspace.edit(|d|{for(keys,text)in values{set(d,&keys,toml_edit::value(text));}});return;
                    }
                    let keys=editor::path(&module,"style");let old=editor::value(self,&keys).and_then(|i|i.as_str().map(str::to_string)).unwrap_or_default();
                    self.workspace.edit(|d|{
                        set(d,&keys,toml_edit::value(editor::recolor(&old,bg,&color)));
                        for key in ["format","right_format"] {if let Some(text)=d.get(key).and_then(Item::as_str){d[key]=toml_edit::value(editor::format_color(text,&module,bg,&color));}}
                    });
                    self.notice="Color changed. Styled groups may share the color with adjacent decorations.".into();
                }
            },
            Action::Palette=>self.picker("Prompt color theme","Changes prompt colors while preserving its content and layout. Terminal colors follow your terminal theme.",editor::PALETTES.iter().enumerate().map(|(i,(name,_))|(name.to_string(),Intent::SetPalette(i))).collect()),
            Action::SetPalette(n)=>{
                let mut modules=self.modules.clone();collect_modules(&self.workspace.doc,&mut modules);
                modules.retain(|m|self.included(m));
                let defaults=self.defaults.clone();
                self.workspace.edit(|d|{
                    for key in ["success_symbol","error_symbol","vimcmd_symbol"]{let path=editor::path("character",key);if lookup(d,&path).is_none(){if let Some(v)=lookup(&defaults,&path){set(d,&path,v.clone());}}}
                    editor::apply_palette(d,n,&modules);
                });self.notice=format!("Prompt colors: {}. Aster’s appearance is unchanged.",editor::PALETTES[n.min(editor::PALETTES.len()-1)].0);
            },
        }
    }
    fn input(&mut self, title: &str, text: String, target: InputTarget) {
        let cursor = text.len();
        self.overlay = Some(Overlay::Input(Input {
            title: title.into(),
            text,
            cursor,
            target,
            error: None,
            help: String::new(),
        }));
    }
    fn load_preset(&mut self) {
        if let Some(name) = self.presets.get(self.selections[2][0]) {
            if self.preset_text.as_ref().is_some_and(|(n, _)| n == name)
                || self.preset_pending.as_ref() == Some(name)
            {
                return;
            }
            self.preset_text = None;
            self.preset_pending = Some(name.clone());
            self.preset_cancelled = false;
            let id = self
                .preset_token
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                + 1;
            if let Some(tx) = &self.preset_tx {
                let _ = tx.send((name.clone(), id));
            }
        }
    }
    fn move_segment(&mut self, delta: isize, placement: bool) {
        self.structural_edit(Some((delta, placement)));
    }
    fn structural_edit(&mut self, operation: Option<(isize, bool)>) {
        if self.screen != 0 {
            self.notice = "Select a prompt part in Edit prompt first.".into();
            return;
        }
        let Some(segment) = self.segments().get(self.selections[0][0]).cloned() else {
            return;
        };
        let Some(module) = segment.module else {
            self.notice = "Use Advanced → Edit format to change custom decorations.".into();
            return;
        };
        if module == "all" {
            self.notice = "Choose individual parts before changing automatic content.".into();
            return;
        }
        let key = if segment.right {
            "right_format"
        } else {
            "format"
        };
        let text = if segment.right {
            self.workspace
                .doc
                .get(key)
                .and_then(Item::as_str)
                .unwrap_or("")
                .into()
        } else {
            config::logical_format(&self.workspace.doc)
        };
        let units = match editor::units(&text) {
            Ok(u) => u,
            Err(e) => {
                self.notice = e;
                return;
            }
        };
        let occurrence = self
            .segments()
            .iter()
            .take(self.selections[0][0])
            .filter(|s| s.right == segment.right && s.module.as_ref() == Some(&module))
            .count();
        let mut count = 0;
        let mut index = None;
        for (i, u) in units.iter().enumerate() {
            for m in &u.modules {
                if m == &module {
                    if count == occurrence {
                        index = Some(i);
                    }
                    count += 1;
                }
            }
        }
        let Some(i) = index else { return };
        let unit = &units[i];
        if unit.modules.len() != 1 {
            self.notice="This part shares a styled or conditional group. Its settings are editable; change the layout in Advanced → Edit format, or create a guided replacement.".into();
            return;
        }
        let mut result = text.clone();
        match operation {
            None => {
                result.replace_range(unit.start..unit.end, "");
            }
            Some((_, true)) => {
                let other = if segment.right {
                    "format"
                } else {
                    "right_format"
                };
                let mut dest = if other == "format" {
                    config::logical_format(&self.workspace.doc)
                } else {
                    self.workspace
                        .doc
                        .get(other)
                        .and_then(Item::as_str)
                        .unwrap_or("")
                        .into()
                };
                dest.push_str(&text[unit.start..unit.end]);
                result.replace_range(unit.start..unit.end, "");
                self.workspace.edit(|d| {
                    d[key] = toml_edit::value(result);
                    d[other] = toml_edit::value(dest);
                });
                self.notice = "Moved to the other side.".into();
                return;
            }
            Some((delta, false)) => {
                let other = if delta < 0 {
                    (0..i).rev().find(|j| !units[*j].modules.is_empty())
                } else {
                    (i + 1..units.len()).find(|j| !units[*j].modules.is_empty())
                };
                let Some(j) = other else {
                    self.notice = "Already at the end of this side.".into();
                    return;
                };
                if units[j].modules.len() != 1 {
                    self.notice="The neighboring part shares a group. Use Advanced → Edit format to reorder it safely.".into();
                    return;
                }
                let u = &units[j];
                let (first, last) = if unit.start < u.start {
                    (unit, u)
                } else {
                    (u, unit)
                };
                let replacement = format!(
                    "{}{}{}",
                    &text[last.start..last.end],
                    &text[first.end..last.start],
                    &text[first.start..first.end]
                );
                result.replace_range(first.start..last.end, &replacement);
                self.selections[0][0] = self.selections[0][0].saturating_add_signed(delta);
            }
        }
        self.workspace.edit(|d| d[key] = toml_edit::value(result));
        self.notice = if operation.is_none() {
            "Removed from prompt. Settings kept; Undo restores the part."
        } else {
            "Prompt order updated."
        }
        .into();
    }
    pub fn on_key(&mut self, key: KeyEvent) {
        if self.overlay.is_some() {
            if matches!(key.code, K::PageDown | K::PageUp) {
                self.overlay_scroll = self
                    .overlay_scroll
                    .saturating_add_signed(if key.code == K::PageDown { 5 } else { -5 });
                return;
            }
            let menu_count = if let Some(Overlay::Menu {
                navigation, query, ..
            }) = &self.overlay
            {
                self.menu(*navigation, query).len()
            } else {
                0
            };
            let picker_count = self.picker_items().len();
            let mut action = None;
            match self.overlay.as_mut().unwrap() {
                Overlay::Picker {
                    selected, query, ..
                } => match key.code {
                    K::Esc => action = Some(Intent::ModalCancel),
                    K::Enter => action = Some(Intent::ModalAccept),
                    K::Up => *selected = selected.saturating_sub(1),
                    K::Down => *selected = (*selected + 1).min(picker_count.saturating_sub(1)),
                    K::Backspace => {
                        query.pop();
                        *selected = 0;
                    }
                    K::Char(c) if !key.modifiers.contains(M::CONTROL) => {
                        query.push(c);
                        *selected = 0;
                    }
                    _ => {}
                },
                Overlay::Input(input) => {
                    action = match key.code {
                        K::Esc => Some(Intent::ModalCancel),
                        K::Enter => Some(Intent::ModalAccept),
                        _ => None,
                    };
                    if action.is_none() {
                        if matches!(input.target, InputTarget::Property(_))
                            && ["true", "false"].contains(&input.text.trim())
                            && matches!(key.code, K::Char(' ') | K::Left | K::Right)
                        {
                            input.text = if input.text.trim() == "true" {
                                "false"
                            } else {
                                "true"
                            }
                            .into();
                            input.cursor = input.text.len();
                        } else {
                            text_key(&mut input.text, &mut input.cursor, key, false);
                        }
                    }
                }
                Overlay::Menu {
                    query, selected, ..
                } => {
                    let count = menu_count.max(1);
                    match key.code {
                        K::Esc => action = Some(Intent::ModalCancel),
                        K::Enter => action = Some(Intent::ModalAccept),
                        K::Up => *selected = selected.saturating_sub(1),
                        K::Down => *selected = (*selected + 1).min(count - 1),
                        K::Backspace => {
                            query.pop();
                            *selected = 0;
                        }
                        K::Char(c) => {
                            query.push(c);
                            *selected = 0;
                        }
                        _ => {}
                    }
                }
                Overlay::Quit => {
                    action = match key.code {
                        K::Char('d') => Some(Intent::ModalDiscard),
                        K::Char('r') => Some(Intent::ModalReview),
                        K::Char('c') | K::Esc => Some(Intent::ModalCancel),
                        _ => None,
                    }
                }
                Overlay::Approval | Overlay::Preset(_) => {
                    action = match key.code {
                        K::Char('a') => Some(Intent::ModalApprove),
                        K::Esc => Some(Intent::ModalCancel),
                        _ => None,
                    }
                }
                Overlay::Help => {
                    if matches!(key.code, K::Esc | K::F(1)) {
                        action = Some(Intent::ModalCancel);
                    }
                }
                Overlay::Wizard => {
                    let wizard = self.wizard.as_mut().unwrap();
                    if wizard.hex.is_some() {
                        match key.code {
                            K::Esc => action = Some(Intent::BuilderBack),
                            K::Enter => action = Some(Intent::BuilderNext),
                            K::Backspace => wizard.hex_backspace(),
                            K::Char(c) if !key.modifiers.contains(M::CONTROL) => wizard.hex_char(c),
                            _ => {}
                        }
                    } else {
                        action = match key.code {
                            K::Char('q') => Some(Intent::Quit),
                            K::Char('c') if key.modifiers.contains(M::CONTROL) => {
                                Some(Intent::Quit)
                            }
                            K::F(6) => Some(Intent::BuilderPreview),
                            K::Up | K::Char('k') => Some(Intent::BuilderMove(-1)),
                            K::Down | K::Char('j') => Some(Intent::BuilderMove(1)),
                            K::Char(' ') => Some(Intent::BuilderToggle),
                            K::Left => Some(Intent::BuilderColor(-1)),
                            K::Right => Some(Intent::BuilderColor(1)),
                            K::Char('[') => Some(Intent::BuilderReorder(-1)),
                            K::Char(']') => Some(Intent::BuilderReorder(1)),
                            K::Char('c') if wizard.step == 3 => Some(Intent::BuilderHex),
                            K::Char('t') => Some(Intent::BuilderText),
                            K::Enter => Some(Intent::BuilderNext),
                            K::Esc => Some(Intent::BuilderBack),
                            K::Char('x') => Some(Intent::BuilderCancel),
                            _ => None,
                        };
                    }
                }
            }
            if let Some(intent) = action {
                if let Intent::Reorder(d) = intent {
                    if let Some(w) = self.wizard.as_mut() {
                        w.move_piece(d);
                    }
                } else {
                    self.dispatch(intent);
                }
            }
            return;
        }
        if self.raw_editing {
            if key.modifiers.contains(M::CONTROL) && key.code == K::Char('s') {
                self.dispatch(Intent::Save);
                return;
            }
            if key.code == K::F(6) {
                self.raw_editing = false;
                self.dispatch(Intent::Pane(1));
                return;
            }
            if key.code == K::Esc {
                self.raw_editing = false;
                return;
            }
            if key.modifiers.contains(M::CONTROL) && key.code == K::Enter {
                self.notice = match self.workspace.apply_raw() {
                    Ok(()) => "TOML applied to shared draft".into(),
                    Err(e) => e,
                };
                return;
            }
            text_key(&mut self.workspace.raw, &mut self.raw_cursor, key, true);
            self.workspace.error = self
                .workspace
                .raw
                .parse::<DocumentMut>()
                .err()
                .map(|e| e.to_string());
            return;
        }
        let ctrl = key.modifiers.contains(M::CONTROL);
        let shift = key.modifiers.contains(M::SHIFT);
        let intent = if ctrl {
            match key.code {
                K::Char('c') => Some(Intent::Quit),
                K::Char('s') => Some(Intent::Save),
                K::Char('z') => Some(Intent::Undo),
                K::Char('y') => Some(Intent::Redo),
                K::Char('k') => Some(Intent::Actions),
                K::Char('x') => Some(Intent::Cancel),
                _ => None,
            }
        } else {
            match key.code {
                K::Tab | K::BackTab | K::F(6) if self.screen == 0 => {
                    Some(Intent::Pane(if self.pane() == 0 { 1 } else { 0 }))
                }
                K::Tab => Some(Intent::Screen(0)),
                K::BackTab => Some(Intent::Screen(0)),
                K::F(6) => Some(Intent::Pane(
                    (self.pane()
                        + if shift {
                            PANES[self.screen].len() - 1
                        } else {
                            1
                        })
                        % PANES[self.screen].len(),
                )),
                K::F(2) => Some(Intent::Navigation),
                K::F(1) | K::Char('?') => Some(Intent::Help),
                K::Char('s') => Some(Intent::Save),
                K::Char('u') => Some(Intent::Undo),
                K::Char('U') => Some(Intent::Redo),
                K::Char('a') if self.screen == 0 => Some(Intent::Edit(Action::Add)),
                K::Char('a') if self.screen == 2 => Some(Intent::AdoptPreset),
                K::Char('p') => Some(Intent::Screen(2)),
                K::Char('t') => Some(Intent::Screen(3)),
                K::Char('b') => Some(Intent::Screen(0)),
                K::Char('q') => Some(Intent::Quit),
                K::Char('1') => Some(Intent::Screen(0)),
                K::Char('2') => Some(Intent::Screen(2)),
                K::Char('3') => Some(Intent::Advanced),
                K::Up | K::Char('k') => Some(Intent::Move(-1)),
                K::Down | K::Char('j') => Some(Intent::Move(1)),
                K::PageUp => Some(if self.rows().is_empty() {
                    Intent::Scroll(-10)
                } else {
                    Intent::Move(-10)
                }),
                K::PageDown => Some(if self.rows().is_empty() {
                    Intent::Scroll(10)
                } else {
                    Intent::Move(10)
                }),
                K::Esc if self.screen == 0 => Some(Intent::Pane(0)),
                K::Home if self.screen == 0 => Some(Intent::Edit(Action::WholePrompt)),
                K::Left if self.screen == 0 => Some(Intent::Pane(0)),
                K::Right if self.screen == 0 => Some(Intent::Pane(1)),
                K::Left if self.rows().is_empty() => Some(Intent::ScrollX(-4)),
                K::Right if self.rows().is_empty() => Some(Intent::ScrollX(4)),
                K::Enter => Some(Intent::Activate),
                K::Char(' ') => Some(Intent::Toggle),
                K::Char('/') if self.screen == 1 => Some(Intent::Filter),
                K::Char('f') if self.screen == 1 => Some(Intent::CycleFilter),
                K::Char('n') => Some(Intent::New),
                K::Char('v') => Some(Intent::Scenario),
                K::Char('[') => Some(Intent::Reorder(-1)),
                K::Char(']') => Some(Intent::Reorder(1)),
                K::Char('r') if self.screen == 0 => Some(Intent::Placement),
                K::Delete => Some(Intent::RemoveKey),
                _ => None,
            }
        };
        if let Some(intent) = intent {
            self.dispatch(intent);
        }
    }
    fn modal_action(&mut self, intent: Intent) {
        let Some(mut overlay) = self.overlay.take() else {
            return;
        };
        let mut keep = true;
        match (&mut overlay, intent) {
            (
                Overlay::Picker {
                    options,
                    query,
                    selected,
                    ..
                },
                Intent::ModalAccept,
            ) => {
                let items = options
                    .iter()
                    .filter(|(s, _)| s.to_lowercase().contains(&query.to_lowercase()))
                    .collect::<Vec<_>>();
                if let Some((_, intent)) = items.get((*selected).min(items.len().saturating_sub(1)))
                {
                    self.dispatch(intent.clone());
                }
                return;
            }
            (Overlay::Input(input), Intent::ModalAccept) => match self.submit(input) {
                Ok(()) => keep = false,
                Err(e) => input.error = Some(e),
            },
            (
                Overlay::Menu {
                    navigation,
                    query,
                    selected,
                },
                Intent::ModalAccept,
            ) => {
                let items = self.menu(*navigation, query);
                if let Some((_, intent)) = items.get((*selected).min(items.len().saturating_sub(1)))
                {
                    self.dispatch(intent.clone());
                }
                return;
            }
            (Overlay::Quit, Intent::ModalDiscard) => {
                self.quit = true;
                keep = false;
            }
            (Overlay::Quit, Intent::ModalReview) => {
                self.wizard = None;
                self.dispatch(Intent::Review);
                keep = false;
            }
            (Overlay::Quit, Intent::ModalCancel) => {
                if self.wizard.is_some() {
                    self.overlay = Some(Overlay::Wizard);
                }
                return;
            }
            (Overlay::Approval, Intent::ModalApprove) => {
                self.approved = preview::custom_definitions(&self.workspace.doc.to_string());
                self.notice = "Current custom definitions approved for this session".into();
                keep = false;
            }
            (Overlay::Preset(_), Intent::ModalApprove) => {
                if let Some((_, text)) = &self.preset_text {
                    if let Err(e) = self.workspace.adopt(text.clone()) {
                        self.notice = e;
                    }
                }
                keep = false;
            }
            (Overlay::Wizard, action) => {
                if let Some(wizard) = self.wizard.as_mut() {
                    match action {
                        Intent::BuilderNext => {
                            if wizard.hex.is_some() {
                                if let Err(e) = wizard.commit_hex() {
                                    self.notice = e;
                                }
                            } else if wizard.is_confirm() {
                                let text = wizard.draft().to_toml();
                                let _ = self.workspace.adopt(text);
                                self.wizard = None;
                                keep = false;
                            } else {
                                wizard.advance();
                            }
                        }
                        Intent::BuilderBack => {
                            if wizard.hex.is_some() {
                                wizard.hex = None;
                            } else if wizard.step == 0 {
                                self.wizard = None;
                                keep = false;
                            } else {
                                wizard.back();
                            }
                        }
                        Intent::BuilderCancel | Intent::ModalCancel => {
                            self.wizard = None;
                            keep = false;
                        }
                        Intent::BuilderMove(delta) => wizard.move_cursor(delta),
                        Intent::BuilderReorder(delta) => wizard.move_piece(delta),
                        Intent::BuilderSelect(n) => {
                            wizard.move_cursor(n as isize - wizard.cursor() as isize)
                        }
                        Intent::BuilderToggle => wizard.toggle_piece(),
                        Intent::BuilderColor(d) => wizard.cycle_color(d),
                        Intent::BuilderHex => wizard.start_hex(),
                        Intent::BuilderText => wizard.flip_text(),
                        Intent::BuilderPreview => self.wizard_preview = !self.wizard_preview,
                        _ => {}
                    }
                }
            }
            (_, Intent::ModalCancel) => keep = false,
            _ => {}
        }
        if keep {
            self.overlay = Some(overlay);
        } else {
            self.overlay_scroll = 0;
        }
    }
    fn submit(&mut self, input: &Input) -> Result<(), String> {
        match &input.target {
            InputTarget::Plain(keys) | InputTarget::Number(keys) => {
                if self.workspace.error.is_some() || self.workspace.load_error.is_some() {
                    return Err("Correct the source before editing settings.".into());
                }
                let item = if matches!(input.target, InputTarget::Number(_)) {
                    let n = input
                        .text
                        .trim()
                        .parse::<i64>()
                        .map_err(|_| "Enter a whole number, such as 2000.")?;
                    if n < 0 {
                        return Err("Enter zero or a positive number.".into());
                    }
                    toml_edit::value(n)
                } else {
                    let old = editor::value(self, keys)
                        .and_then(|i| i.as_str().map(str::to_string))
                        .unwrap_or_default();
                    let text = if keys.last().is_some_and(|k| k.ends_with("symbol")) {
                        editor::replace_symbol(&old, &input.text)
                    } else {
                        input.text.clone()
                    };
                    toml_edit::value(text)
                };
                self.workspace.edit(|d| set(d, keys, item));
            }
            InputTarget::Color(bg) => {
                let color = input.text.trim();
                if color.len() != 7
                    || !color.starts_with('#')
                    || !color[1..].bytes().all(|b| b.is_ascii_hexdigit())
                {
                    return Err("Use six hex digits, for example #7dcfff.".into());
                }
                self.editor_action(Action::SetColor(*bg, color.into()));
            }
            InputTarget::Search => {
                self.filter = input.text.clone();
                self.selections[1][0] = 0;
            }
            InputTarget::SaveAs => {
                if input.text.trim().is_empty() {
                    return Err("Choose a new target path".into());
                }
                self.workspace
                    .save(Some(PathBuf::from(input.text.trim())))
                    .map_err(|e| format!("{e:#}"))?;
                self.notice = "Saved to new target".into();
            }
            InputTarget::Left | InputTarget::Right => {
                if self.workspace.error.is_some() {
                    return Err("Apply or correct the TOML draft first".into());
                }
                let key = if matches!(input.target, InputTarget::Left) {
                    "format"
                } else {
                    "right_format"
                };
                self.workspace
                    .edit(|d| d[key] = toml_edit::value(&input.text));
            }
            InputTarget::Property(keys) => {
                if self.workspace.error.is_some() || self.workspace.load_error.is_some() {
                    return Err("Correct the source in TOML first".into());
                }
                let (keys, value) = if keys.is_empty() {
                    let (path, value) =
                        input.text.split_once('=').ok_or("Use path = TOML value")?;
                    (
                        path.trim()
                            .split('.')
                            .map(str::to_string)
                            .collect::<Vec<_>>(),
                        value.trim().to_string(),
                    )
                } else {
                    (keys.clone(), input.text.clone())
                };
                if keys.iter().any(|s| s.is_empty()) {
                    return Err("Empty property path".into());
                }
                let parsed = format!("value = {value}")
                    .parse::<DocumentMut>()
                    .map_err(|e| e.to_string())?;
                let item = parsed.get("value").ok_or("Missing TOML value")?.clone();
                let field = keys.last().unwrap();
                let known = if keys.len() == 1 {
                    config::PROMPT_FIELDS.contains(&field.as_str()) || field == "follow_symlinks"
                } else {
                    self.modules.contains(&keys[0]) || keys[0] == "custom" || keys[0] == "env_var"
                };
                if known
                    && ["disabled", "add_newline", "show_always", "ssh_only"]
                        .contains(&field.as_str())
                    && item.as_bool().is_none()
                {
                    return Err("This property needs true or false".into());
                }
                if known
                    && [
                        "scan_timeout",
                        "command_timeout",
                        "min_time",
                        "truncation_length",
                    ]
                    .contains(&field.as_str())
                    && item.as_integer().is_none()
                {
                    return Err("This property needs an integer".into());
                }
                self.workspace.edit(|d| set(d, &keys, item));
            }
            InputTarget::Scenario => {
                let parts = input.text.split('|').map(str::trim).collect::<Vec<_>>();
                if parts.len() != 5 {
                    return Err("Use directory | width | status | duration ms | jobs".into());
                }
                let width = parts[1].parse::<u16>().map_err(|_| "Invalid width")?;
                if width == 0 {
                    return Err("Width must be positive".into());
                }
                self.scenario = Scenario {
                    index: self.scenario.index,
                    directory: parts[0].into(),
                    width,
                    status: parts[2].parse().map_err(|_| "Invalid exit status")?,
                    duration: parts[3].parse().map_err(|_| "Invalid duration")?,
                    jobs: parts[4].parse().map_err(|_| "Invalid job count")?,
                };
            }
        }
        Ok(())
    }
    pub fn mouse(&mut self, event: MouseEvent) {
        let hit = self
            .hits
            .iter()
            .rev()
            .find(|h| h.area.contains((event.column, event.row).into()))
            .cloned();
        if let Some(hit) = hit {
            match event.kind {
                MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                    if self.overlay.is_some() {
                        if let Intent::Select(n) = hit.intent {
                            if let Some(
                                Overlay::Menu { selected, .. } | Overlay::Picker { selected, .. },
                            ) = &mut self.overlay
                            {
                                *selected = n;
                            }
                            if matches!(self.overlay, Some(Overlay::Picker { .. })) {
                                self.dispatch(Intent::ModalAccept);
                            }
                        } else {
                            self.dispatch(hit.intent);
                        }
                        return;
                    }
                    if let Intent::FocusSelect(p, n) = hit.intent {
                        self.dispatch(Intent::Pane(p));
                        let double = self.last_click.is_some_and(|(screen, pane, old, t)| {
                            screen == self.screen
                                && pane == p
                                && old == n
                                && t.elapsed() < Duration::from_millis(350)
                        });
                        self.dispatch(Intent::Select(n));
                        self.last_click = Some((self.screen, p, n, Instant::now()));
                        if double || self.screen == 0 && p == 1 {
                            self.dispatch(Intent::Activate);
                        }
                    } else {
                        self.dispatch(hit.intent);
                    }
                }
                MouseEventKind::ScrollDown | MouseEventKind::ScrollUp => {
                    if self.overlay.is_some() {
                        let d = if event.kind == MouseEventKind::ScrollDown {
                            3
                        } else {
                            -3
                        };
                        let max = self.picker_items().len().saturating_sub(1);
                        let is_picker = matches!(self.overlay, Some(Overlay::Picker { .. }));
                        if matches!(self.overlay, Some(Overlay::Wizard)) {
                            self.dispatch(Intent::BuilderMove(d));
                        } else if let Some(
                            Overlay::Menu { selected, .. } | Overlay::Picker { selected, .. },
                        ) = &mut self.overlay
                        {
                            *selected = selected.saturating_add_signed(d);
                            if is_picker {
                                *selected = (*selected).min(max);
                            }
                        } else {
                            self.dispatch(Intent::Scroll(d));
                        }
                        return;
                    }
                    match hit.intent {
                        Intent::Pane(p) | Intent::FocusSelect(p, _) => {
                            self.dispatch(Intent::Pane(p))
                        }
                        _ => {}
                    }
                    let d = if event.kind == MouseEventKind::ScrollDown {
                        3
                    } else {
                        -3
                    };
                    self.dispatch(if self.rows().is_empty() {
                        Intent::Scroll(d)
                    } else {
                        Intent::Move(d)
                    });
                }
                _ => {}
            }
        }
    }
}
impl Drop for App {
    fn drop(&mut self) {
        self.preview.cancel();
        self.preset_token
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.preset_tx.take();
        if let Some(worker) = self.preset_worker.take() {
            let _ = worker.join();
        }
    }
}
pub fn lookup<'a>(doc: &'a DocumentMut, keys: &[String]) -> Option<&'a Item> {
    let (first, rest) = keys.split_first()?;
    let mut item = doc.get(first)?;
    for key in rest {
        item = item.as_table_like()?.get(key)?;
    }
    Some(item)
}
pub fn set(doc: &mut DocumentMut, keys: &[String], value: Item) {
    fn descend(table: &mut dyn toml_edit::TableLike, keys: &[String], value: Item) {
        if keys.len() == 1 {
            let mut value = value;
            if let (Some(old), Some(new)) = (
                table.get(&keys[0]).and_then(Item::as_value),
                value.as_value_mut(),
            ) {
                *new.decor_mut() = old.decor().clone();
            }
            table.insert(&keys[0], value);
            return;
        }
        if !table.contains_key(&keys[0]) {
            table.insert(&keys[0], toml_edit::table());
        }
        if let Some(child) = table.get_mut(&keys[0]).and_then(Item::as_table_like_mut) {
            descend(child, &keys[1..], value);
        }
    }
    if !keys.is_empty() {
        descend(doc.as_table_mut(), keys, value);
    }
}
fn remove(doc: &mut DocumentMut, keys: &[String]) {
    if keys.len() == 1 {
        doc.remove(&keys[0]);
        return;
    }
    let mut item = doc.get_mut(&keys[0]);
    for key in &keys[1..keys.len() - 1] {
        item = item
            .and_then(Item::as_table_like_mut)
            .and_then(|t| t.get_mut(key));
    }
    if let Some(t) = item.and_then(Item::as_table_like_mut) {
        t.remove(keys.last().unwrap());
    }
}
fn collect_modules(doc: &DocumentMut, names: &mut Vec<String>) {
    for (name, item) in doc.iter() {
        if item.as_table_like().is_some() {
            if name == "custom" || name == "env_var" {
                if let Some(t) = item.as_table_like() {
                    for (n, i) in t.iter() {
                        if i.as_table_like().is_some() {
                            names.push(format!("{name}.{n}"));
                        }
                    }
                }
            } else if name != "palettes" && name != "profiles" {
                names.push(name.into());
            }
        }
    }
}
fn flatten_properties(
    doc: &DocumentMut,
    prefix: &[String],
    rows: &mut Vec<(Vec<String>, String, String, bool)>,
    explicit: bool,
) {
    fn walk(
        table: &dyn toml_edit::TableLike,
        path: &[String],
        rows: &mut Vec<(Vec<String>, String, String, bool)>,
        explicit: bool,
        base: usize,
    ) {
        for (key, item) in table.iter() {
            let mut p = path.to_vec();
            p.push(key.into());
            if let Some(t) = item.as_table_like() {
                walk(t, &p, rows, explicit, base);
            } else if item.is_value() {
                rows.retain(|(old, _, _, _)| old != &p);
                rows.push((
                    p.clone(),
                    p[base..].join("."),
                    item.to_string().trim().to_string(),
                    explicit,
                ));
            }
        }
    }
    if prefix.is_empty() {
        for (key, item) in doc.iter() {
            if item.is_value() {
                let p = vec![key.into()];
                rows.retain(|(old, _, _, _)| old != &p);
                rows.push((p, key.into(), item.to_string().trim().into(), explicit));
            }
        }
    } else if let Some(t) = lookup(doc, prefix).and_then(Item::as_table_like) {
        walk(t, prefix, rows, explicit, prefix.len());
    }
}
fn preference_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("aster/appearance")
}
pub fn text_key(text: &mut String, cursor: &mut usize, key: KeyEvent, multiline: bool) {
    *cursor = (*cursor).min(text.len());
    while !text.is_char_boundary(*cursor) {
        *cursor = cursor.saturating_sub(1);
    }
    let prev = |s: &str, c: usize| {
        s[..c]
            .char_indices()
            .next_back()
            .map(|(i, _)| i)
            .unwrap_or(0)
    };
    let next = |s: &str, c: usize| {
        s[c..]
            .chars()
            .next()
            .map(|ch| c + ch.len_utf8())
            .unwrap_or(c)
    };
    match key.code {
        K::Left => *cursor = prev(text, *cursor),
        K::Right => *cursor = next(text, *cursor),
        K::Home => *cursor = text[..*cursor].rfind('\n').map(|n| n + 1).unwrap_or(0),
        K::End => {
            *cursor = text[*cursor..]
                .find('\n')
                .map(|n| *cursor + n)
                .unwrap_or(text.len())
        }
        K::Up | K::Down if multiline => {
            let start = text[..*cursor].rfind('\n').map(|n| n + 1).unwrap_or(0);
            let column = text[start..*cursor].chars().count();
            let target = if key.code == K::Up {
                if start == 0 {
                    return;
                }
                text[..start - 1].rfind('\n').map(|n| n + 1).unwrap_or(0)
            } else {
                let Some(n) = text[*cursor..].find('\n') else {
                    return;
                };
                *cursor + n + 1
            };
            let line = text[target..].split('\n').next().unwrap_or("");
            *cursor = target
                + line
                    .char_indices()
                    .nth(column)
                    .map(|(i, _)| i)
                    .unwrap_or(line.len());
        }
        K::Backspace => {
            let p = prev(text, *cursor);
            text.replace_range(p..*cursor, "");
            *cursor = p;
        }
        K::Delete => {
            let n = next(text, *cursor);
            text.replace_range(*cursor..n, "");
        }
        K::Enter if multiline => {
            text.insert(*cursor, '\n');
            *cursor += 1;
        }
        K::Tab => {
            text.insert_str(*cursor, "    ");
            *cursor += 4;
        }
        K::Char(c) if !key.modifiers.contains(M::CONTROL) => {
            text.insert(*cursor, c);
            *cursor += c.len_utf8();
        }
        _ => {}
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn app() -> App {
        let dir = tempfile::tempdir().unwrap();
        App::new(dir.path().join("missing.toml"), false, false)
    }
    #[test]
    fn grouped_add_and_enable_all() {
        let dir = tempfile::tempdir().unwrap();
        let mut a = App::new(dir.path().join("prompt.toml"), false, false);
        a.workspace.adopt("# keep\nformat = '[✦](cyan)$directory$character'\nright_format = '$git_branch'\n[python]\ndisabled = true\n[custom.example]\ncommand = 'printf hello'\n".into()).unwrap();
        a.dispatch(Intent::Edit(Action::Add));
        let items = a.picker_items();
        assert!(items
            .iter()
            .any(|(label, _)| label.starts_with("Programming languages ·")));
        assert!(items
            .iter()
            .any(|(label, _)| label.starts_with("Enable all ·")));
        assert!(!items.iter().any(
            |(_, action)| matches!(action,Intent::Edit(Action::Insert(m))if m=="rust"||m=="python")
        ));
        a.overlay = None;
        let before = a.workspace.raw.clone();
        a.dispatch(Intent::Edit(Action::InsertFamily(0)));
        assert!(a.included("rust"));
        assert!(a.included("python"));
        assert!(a.enabled("python"));
        let after = a.workspace.raw.clone();
        a.dispatch(Intent::Edit(Action::InsertFamily(0)));
        assert_eq!(a.workspace.raw, after);
        a.dispatch(Intent::Undo);
        assert_eq!(a.workspace.raw, before);
        a.dispatch(Intent::Edit(Action::EnableAll));
        for m in a.addable_modules() {
            assert!(a.included(&m), "not included: {m}");
            assert!(a.enabled(&m), "not enabled: {m}");
        }
        assert_eq!(
            a.workspace.doc["right_format"].as_str(),
            Some("$git_branch")
        );
        assert_eq!(
            config::find_vars(&config::logical_format(&a.workspace.doc), "git_branch").len(),
            0
        );
        assert!(a.workspace.raw.contains("# keep"));
        assert_eq!(
            a.workspace.doc["custom"]["example"]["command"].as_str(),
            Some("printf hello")
        );
        a.dispatch(Intent::Undo);
        assert_eq!(a.workspace.raw, before);
        a.workspace.edit(|d| d["format"] = toml_edit::value("$all"));
        a.dispatch(Intent::Edit(Action::EnableAll));
        assert_eq!(config::logical_format(&a.workspace.doc), "$all");
    }
    #[test]
    fn simple_editor_starship_render() {
        let dir = tempfile::tempdir().unwrap();
        let mut a = App::new(dir.path().join("prompt.toml"), false, false);
        a.workspace
            .adopt(
                crate::presets::get("Aster · Aurora Powerline")
                    .unwrap()
                    .to_string(),
            )
            .unwrap();
        a.dispatch(Intent::SetPalette(0));
        a.selections[0][0] = a
            .segments()
            .iter()
            .position(|s| s.module.as_deref() == Some("directory"))
            .unwrap();
        a.dispatch(Intent::SetColor(true, "black".into()));
        a.selections[0][0] = a
            .segments()
            .iter()
            .position(|s| s.module.as_deref() == Some("character"))
            .unwrap();
        a.dispatch(Intent::SetColor(false, "cyan".into()));
        let config = dir.path().join("preview.toml");
        std::fs::write(&config, &a.workspace.raw).unwrap();
        let mut command = std::process::Command::new("starship");
        command
            .args([
                "prompt",
                "--path",
                dir.path().to_str().unwrap(),
                "--status",
                "1",
                "--cmd-duration",
                "4200",
            ])
            .env("STARSHIP_CONFIG", &config)
            .env("TERM", "xterm-256color");
        let text = preview::bounded(command, None).unwrap();
        assert!(text.contains("×"));
        assert!(!text.contains("Unknown"));
    }
    #[test]
    fn simple_editor_workflow() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("prompt.toml");
        let mut a = App::new(target.clone(), false, false);
        a.workspace
            .adopt(crate::presets::get("Aster · Aurora").unwrap().to_string())
            .unwrap();
        let original = config::logical_format(&a.workspace.doc);
        assert!(a
            .segments()
            .iter()
            .any(|s| s.module.as_deref() == Some("directory")));
        a.selections[0][0] = a
            .segments()
            .iter()
            .position(|s| s.module.as_deref() == Some("directory"))
            .unwrap();
        a.dispatch(Intent::Edit(Action::Group(3)));
        a.dispatch(Intent::Edit(Action::Number(editor::path(
            "directory",
            "truncation_length",
        ))));
        if let Some(Overlay::Input(i)) = &mut a.overlay {
            i.text = "5".into();
        }
        a.dispatch(Intent::ModalAccept);
        assert_eq!(
            lookup(
                &a.workspace.doc,
                &editor::path("directory", "truncation_length")
            )
            .and_then(Item::as_integer),
            Some(5)
        );
        assert_eq!(config::logical_format(&a.workspace.doc), original);
        a.dispatch(Intent::SetColor(false, "cyan".into()));
        assert_eq!(config::logical_format(&a.workspace.doc), original);
        assert_eq!(
            lookup(&a.workspace.doc, &editor::path("directory", "style")).and_then(Item::as_str),
            Some("bold fg:cyan")
        );
        a.dispatch(Intent::Edit(Action::Insert("rust".into())));
        assert!(a.included("rust"));
        a.dispatch(Intent::Edit(Action::Remove));
        assert!(!a.included("rust"));
        a.dispatch(Intent::Undo);
        assert!(a.included("rust"));
        a.workspace
            .edit(|d| d["format"] = toml_edit::value("[($directory$git_branch)](blue)$character"));
        a.selections[0][0] = 0;
        let grouped = a.workspace.raw.clone();
        a.dispatch(Intent::Edit(Action::Remove));
        assert_eq!(a.workspace.raw, grouped);
        assert!(a.notice.contains("shares"));
        a.dispatch(Intent::SetPalette(0));
        assert!(a.workspace.raw.contains("cyan"));
        assert!(!a.workspace.raw.contains("#b4a6ed"));
        a.selections[0][0] = a
            .segments()
            .iter()
            .position(|s| s.module.as_deref() == Some("character"))
            .unwrap();
        a.dispatch(Intent::Edit(Action::Text(editor::path(
            "character",
            "success_symbol",
        ))));
        if let Some(Overlay::Input(i)) = &mut a.overlay {
            assert!(!i.text.contains("bold"));
            i.text = "→".into();
        }
        a.dispatch(Intent::ModalAccept);
        assert!(lookup(
            &a.workspace.doc,
            &editor::path("character", "success_symbol")
        )
        .unwrap()
        .as_str()
        .unwrap()
        .contains("→"));
        a.dispatch(Intent::SetColor(false, "purple".into()));
        assert!(a.workspace.doc["character"].get("style").is_none());
        a.on_key(KeyEvent::new(K::Char('s'), M::CONTROL));
        assert_eq!(a.screen, 0);
        assert!(!a.workspace.dirty());
        assert!(target.exists());
        a.workspace
            .edit(|d| d["add_newline"] = toml_edit::value(false));
        std::fs::write(&target, "# externally changed\n").unwrap();
        let draft = a.workspace.raw.clone();
        a.dispatch(Intent::Save);
        assert!(a.notice.contains("changed outside"));
        assert_eq!(a.workspace.raw, draft);
        assert!(a.workspace.dirty());
    }
    #[test]
    fn shared_workspace_and_staged_cancel() {
        let mut a = app();
        a.workspace
            .edit(|d| d["format"] = toml_edit::value("$rust"));
        let before = a.workspace.raw.clone();
        a.dispatch(Intent::New);
        a.on_key(KeyEvent::new(K::Char('x'), M::NONE));
        assert_eq!(a.workspace.raw, before);
        a.dispatch(Intent::Screen(1));
        assert!(a.included("rust"));
        a.dispatch(Intent::Undo);
        assert_eq!(a.workspace.raw, "");
    }
    #[test]
    fn nested_custom_and_empty_value() {
        let mut d = DocumentMut::new();
        set(
            &mut d,
            &["custom".into(), "test".into(), "command".into()],
            toml_edit::value(""),
        );
        assert_eq!(
            lookup(&d, &["custom".into(), "test".into(), "command".into()])
                .unwrap()
                .as_str(),
            Some("")
        );
        remove(&mut d, &["custom".into(), "test".into(), "command".into()]);
        assert!(lookup(&d, &["custom".into(), "test".into(), "command".into()]).is_none());
    }
    #[test]
    fn input_precedes_shortcuts() {
        let mut a = app();
        a.dispatch(Intent::EditLeft);
        for c in "q?123".chars() {
            a.on_key(KeyEvent::new(K::Char(c), M::NONE));
        }
        assert!(!a.quit);
        assert_eq!(a.screen, 0);
        if let Some(Overlay::Input(i)) = &a.overlay {
            assert!(i.text.ends_with("q?123"));
        } else {
            panic!();
        }
    }
    #[test]
    fn complex_format_is_lossless() {
        let mut a = app();
        let format = "[🌌 $directory](blue)${custom.test}";
        a.workspace.edit(|d| d["format"] = toml_edit::value(format));
        assert_eq!(a.segments()[0].module.as_deref(), Some("directory"));
        assert_eq!(a.segments()[1].module.as_deref(), Some("custom.test"));
        a.dispatch(Intent::Edit(Action::Toggle(editor::path(
            "directory",
            "disabled",
        ))));
        assert_eq!(config::logical_format(&a.workspace.doc), format);
    }
    #[test]
    fn unicode_cursor() {
        let mut s = "a界🚀".to_string();
        let mut c = s.len();
        text_key(&mut s, &mut c, KeyEvent::new(K::Backspace, M::NONE), false);
        assert_eq!(s, "a界");
    }
    #[test]
    fn preset_staging_cancel_and_undo() {
        let mut a = app();
        a.workspace
            .edit(|d| d["format"] = toml_edit::value("$directory"));
        let before = a.workspace.raw.clone();
        a.preset_text = Some(("fixture".into(), "format = '$character'\n".into()));
        a.overlay = Some(Overlay::Preset("fixture".into()));
        a.dispatch(Intent::ModalCancel);
        assert_eq!(a.workspace.raw, before);
        a.overlay = Some(Overlay::Preset("fixture".into()));
        a.dispatch(Intent::ModalApprove);
        assert_eq!(a.workspace.doc["format"].as_str(), Some("$character"));
        a.dispatch(Intent::Undo);
        assert_eq!(a.workspace.raw, before);
    }
    #[test]
    fn focus_and_filter_restore() {
        let mut a = app();
        a.screen = 1;
        a.panes[1] = 1;
        a.selections[1][1] = 5;
        a.scrolls[1][1] = 8;
        a.filter = "rust".into();
        a.dispatch(Intent::Screen(0));
        a.dispatch(Intent::Screen(1));
        assert_eq!(a.pane(), 1);
        assert_eq!(a.selected(), 5);
        assert_eq!(a.scrolls[1][1], 8);
        assert_eq!(a.filter, "rust");
    }
    #[test]
    fn variable_boundaries_and_custom_inclusion() {
        let mut a = app();
        a.workspace
            .edit(|d| d["format"] = toml_edit::value("$cmd_duration${custom.one}\\$rust"));
        assert!(!a.included("c"));
        assert!(!a.included("rust"));
        assert!(a.included("cmd_duration"));
        assert!(a.included("custom.one"));
        assert!(!a.included("custom.two"));
    }
}
