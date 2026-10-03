//! Screen state and key handling.

use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::config::{self, field_value, set_field, set_os_symbol, ConfigDoc};
use crate::preview::Previewer;
use crate::theme::Theme;
use crate::wizard::{Catalog, Wizard};

pub enum Screen {
    Wizard,
    Editor,
}

enum Row {
    Prompt,
    Os,
    Module(String),
}

struct Edit {
    buffer: String,
    cursor: usize,
}

pub struct Editor {
    doc: ConfigDoc,
    snapshot: String,
    backup_on_save: bool,
    rows: Vec<Row>,
    filter: String,
    filtering: bool,
    selected: usize,
    field: usize,
    editing: Option<Edit>,
    os_mode: bool,
    os_names: Vec<String>,
    os_selected: usize,
    modules: Vec<String>,
}

impl Editor {
    fn new(modules: Vec<String>) -> Self {
        Self {
            doc: ConfigDoc::parse(""),
            snapshot: String::new(),
            backup_on_save: false,
            rows: Vec::new(),
            filter: String::new(),
            filtering: false,
            selected: 0,
            field: 0,
            editing: None,
            os_mode: false,
            os_names: Vec::new(),
            os_selected: 0,
            modules,
        }
    }

    fn open_clean(&mut self, text: &str) {
        let doc = ConfigDoc::parse(text);
        self.snapshot = doc.to_toml();
        self.doc = doc;
        self.backup_on_save = false;
        self.reset_nav();
    }

    fn open_draft(&mut self, doc: ConfigDoc, original: &str) {
        let saved = ConfigDoc::parse(original).to_toml();
        self.snapshot = saved;
        self.doc = doc;
        self.backup_on_save = self.dirty();
        self.reset_nav();
    }

    fn open_saved_draft(&mut self, doc: ConfigDoc) {
        self.snapshot = doc.to_toml();
        self.doc = doc;
        self.backup_on_save = false;
        self.reset_nav();
    }

    fn reset_nav(&mut self) {
        self.filtering = false;
        self.filter.clear();
        self.selected = 0;
        self.field = 0;
        self.editing = None;
        self.os_mode = false;
        self.os_selected = 0;
        self.rebuild();
    }

    fn dirty(&self) -> bool {
        self.doc.to_toml() != self.snapshot
    }

    fn mark_saved(&mut self) {
        self.snapshot = self.doc.to_toml();
        self.backup_on_save = false;
    }

    fn rebuild(&mut self) {
        if self.os_mode {
            let filter = self.filter.to_lowercase();
            self.os_names = config::os_symbols(&self.doc.doc)
                .into_iter()
                .map(|(name, _)| name)
                .filter(|name| filter.is_empty() || name.to_lowercase().contains(&filter))
                .collect();
            if self.os_selected >= self.os_names.len() {
                self.os_selected = self.os_names.len().saturating_sub(1);
            }
            return;
        }
        let mut modules = self.modules.clone();
        for (key, item) in self.doc.doc.iter() {
            if item.is_table()
                && key != "palettes"
                && key != "profiles"
                && !modules.iter().any(|name| name == key)
            {
                modules.push(key.to_string());
            }
        }
        modules.sort();
        let filter = self.filter.to_lowercase();
        self.rows = vec![Row::Prompt, Row::Os];
        for name in modules {
            if filter.is_empty() || name.to_lowercase().contains(&filter) {
                self.rows.push(Row::Module(name));
            }
        }
        if self.selected >= self.rows.len() {
            self.selected = self.rows.len().saturating_sub(1);
        }
    }

    fn fields(&self) -> &'static [&'static str] {
        match self.rows.get(self.selected) {
            Some(Row::Prompt) => config::PROMPT_FIELDS,
            Some(Row::Module(_)) => config::MODULE_FIELDS,
            _ => &[],
        }
    }

    fn selected_module(&self) -> Option<&str> {
        match self.rows.get(self.selected) {
            Some(Row::Module(name)) => Some(name.as_str()),
            _ => None,
        }
    }
}

pub struct App {
    pub theme: Theme,
    pub screen: Screen,
    pub help: bool,
    pub confirm_quit: bool,
    pub quit: bool,
    pub notice: String,
    pub path: PathBuf,
    user_text: String,
    wizard: Wizard,
    editor: Editor,
    pub preview: Previewer,
}

pub struct Pane {
    pub title: String,
    pub config: String,
    pub selected: bool,
}

impl App {
    pub fn new(path: PathBuf, user_text: String, catalog: Catalog, edit: bool) -> Self {
        let modules = catalog.modules.clone();
        let wizard = Wizard::new(user_text.clone(), crate::wizard::load_swatches());
        let mut editor = Editor::new(modules);
        editor.open_clean(&user_text);
        let notice = catalog.starship_error.unwrap_or_default();
        Self {
            theme: crate::theme::load_omarchy().unwrap_or_else(Theme::terminal),
            screen: if edit { Screen::Editor } else { Screen::Wizard },
            help: false,
            confirm_quit: false,
            quit: false,
            notice,
            path,
            user_text,
            wizard,
            editor,
            preview: Previewer::new(),
        }
    }

    pub fn on_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }
        if self.help {
            self.help = false;
            return;
        }
        if self.confirm_quit {
            match key.code {
                KeyCode::Char('s') => {
                    if self.save_editor() {
                        self.quit = true;
                    }
                    self.confirm_quit = false;
                }
                KeyCode::Char('d') => self.quit = true,
                KeyCode::Esc => self.confirm_quit = false,
                _ => {}
            }
            return;
        }
        match self.screen {
            Screen::Wizard => self.wizard_key(key),
            Screen::Editor => self.editor_key(key),
        }
    }

    pub fn panes(&mut self) -> Vec<Pane> {
        match self.screen {
            Screen::Wizard => self.wizard_panes(),
            Screen::Editor => vec![Pane {
                title: "Preview".into(),
                config: self.editor.doc.to_toml(),
                selected: true,
            }],
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn wizard_panes(&mut self) -> Vec<Pane> {
        let title = "Preview";
        vec![Pane {
            title: title.into(),
            config: self.wizard.preview(),
            selected: true,
        }]
    }

    fn wizard_key(&mut self, key: KeyEvent) {
        if self.wizard.hex.is_some() {
            match key.code {
                KeyCode::Enter => {
                    if let Err(err) = self.wizard.commit_hex() {
                        self.notice = err;
                    } else {
                        self.notice.clear();
                    }
                }
                KeyCode::Esc => {
                    self.wizard.back();
                    if self.notice.starts_with("use a color") {
                        self.notice.clear();
                    }
                }
                KeyCode::Backspace => self.wizard.hex_backspace(),
                KeyCode::Char(ch) => self.wizard.hex_char(ch),
                _ => {}
            }
            return;
        }
        match key.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('s') if self.wizard.is_confirm() => self.save_wizard(),
            KeyCode::Char('e') if self.wizard.is_confirm() => self.open_editor_draft(),
            KeyCode::Char('b') | KeyCode::Esc => {
                if !self.wizard.back() {
                    self.quit = true;
                }
            }
            KeyCode::Left if self.wizard.step == 3 => self.wizard.cycle_color(-1),
            KeyCode::Right if self.wizard.step == 3 => self.wizard.cycle_color(1),
            KeyCode::Char('h') if self.wizard.step == 3 => self.wizard.cycle_color(-1),
            KeyCode::Char('l') if self.wizard.step == 3 => self.wizard.cycle_color(1),
            KeyCode::Left => {
                if !self.wizard.back() {
                    self.quit = true;
                }
            }
            KeyCode::Up | KeyCode::Char('k') => self.wizard.move_cursor(-1),
            KeyCode::Down | KeyCode::Char('j') => self.wizard.move_cursor(1),
            KeyCode::Char(' ') if self.wizard.step == 2 => self.wizard.toggle_piece(),
            KeyCode::Char('[') if self.wizard.step == 2 => self.wizard.move_piece(-1),
            KeyCode::Char(']') if self.wizard.step == 2 => self.wizard.move_piece(1),
            KeyCode::Char('c') if self.wizard.step == 3 => self.wizard.start_hex(),
            KeyCode::Char('t') if self.wizard.step == 3 => self.wizard.flip_text(),
            KeyCode::Enter if !self.wizard.is_confirm() => {
                self.wizard.advance();
            }
            _ => {}
        }
    }

    fn editor_key(&mut self, key: KeyEvent) {
        if self.editor.filtering {
            self.filter_key(key);
            return;
        }
        if self.editor.editing.is_some() {
            self.edit_key(key);
            return;
        }
        match key.code {
            KeyCode::Char('q') => self.request_quit(),
            KeyCode::Char('s') => {
                self.save_editor();
            }
            KeyCode::Char('/') => {
                self.editor.filtering = true;
                self.editor.filter.clear();
                self.editor.rebuild();
            }
            KeyCode::Esc => {
                if self.editor.os_mode {
                    self.editor.os_mode = false;
                    self.editor.filter.clear();
                    self.editor.rebuild();
                }
            }
            KeyCode::Up | KeyCode::Char('k') => self.move_editor(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_editor(1),
            KeyCode::Tab | KeyCode::BackTab => {
                self.cycle_field(if key.code == KeyCode::BackTab { -1 } else { 1 })
            }
            KeyCode::Char(' ') => self.toggle_space(),
            KeyCode::Enter => self.begin_edit(),
            _ => {}
        }
    }

    fn filter_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.editor.filtering = false;
                self.editor.filter.clear();
                self.editor.rebuild();
            }
            KeyCode::Enter => {
                self.editor.filtering = false;
                self.editor.rebuild();
            }
            KeyCode::Backspace => {
                self.editor.filter.pop();
                self.editor.rebuild();
            }
            KeyCode::Char(ch) => {
                self.editor.filter.push(ch);
                self.editor.rebuild();
            }
            _ => {}
        }
    }

    fn edit_key(&mut self, key: KeyEvent) {
        let Some(edit) = self.editor.editing.as_mut() else {
            return;
        };
        match key.code {
            KeyCode::Esc => self.editor.editing = None,
            KeyCode::Enter => self.commit_edit(),
            KeyCode::Left => edit.cursor = edit.cursor.saturating_sub(1),
            KeyCode::Right => {
                let len = edit.buffer.chars().count();
                if edit.cursor < len {
                    edit.cursor += 1;
                }
            }
            KeyCode::Backspace => {
                if edit.cursor > 0 {
                    let at = byte_at(&edit.buffer, edit.cursor - 1);
                    let end = byte_at(&edit.buffer, edit.cursor);
                    edit.buffer.replace_range(at..end, "");
                    edit.cursor -= 1;
                }
            }
            KeyCode::Char(ch) => {
                let at = byte_at(&edit.buffer, edit.cursor);
                edit.buffer.insert(at, ch);
                edit.cursor += 1;
            }
            _ => {}
        }
    }

    fn commit_edit(&mut self) {
        let Some(edit) = self.editor.editing.take() else {
            return;
        };
        let result = if self.editor.os_mode {
            if let Some(name) = self.editor.os_names.get(self.editor.os_selected).cloned() {
                set_os_symbol(&mut self.editor.doc.doc, &name, &edit.buffer);
            }
            Ok(())
        } else if let Some(Row::Prompt) = self.editor.rows.get(self.editor.selected) {
            let field = config::PROMPT_FIELDS
                .get(self.editor.field)
                .copied()
                .unwrap_or("format");
            set_field(&mut self.editor.doc.doc, None, field, &edit.buffer)
        } else if let Some(module) = self.editor.selected_module().map(str::to_string) {
            let field = config::MODULE_FIELDS
                .get(self.editor.field)
                .copied()
                .unwrap_or("symbol");
            set_field(&mut self.editor.doc.doc, Some(&module), field, &edit.buffer)
        } else {
            Ok(())
        };
        match result {
            Ok(()) => self.editor.rebuild(),
            Err(err) => {
                self.notice = err;
                self.editor.editing = Some(edit);
            }
        }
    }

    fn begin_edit(&mut self) {
        if self.editor.os_mode {
            let Some(name) = self.editor.os_names.get(self.editor.os_selected).cloned() else {
                return;
            };
            let value = config::os_symbols(&self.editor.doc.doc)
                .into_iter()
                .find(|(os, _)| os == &name)
                .map(|(_, symbol)| symbol)
                .unwrap_or_default();
            self.start_edit(value);
            return;
        }
        match self.editor.rows.get(self.editor.selected) {
            Some(Row::Os) => {
                self.editor.os_mode = true;
                self.editor.os_selected = 0;
                self.editor.filter.clear();
                self.editor.rebuild();
            }
            Some(Row::Prompt) => {
                let field = config::PROMPT_FIELDS
                    .get(self.editor.field)
                    .copied()
                    .unwrap_or("format");
                let value = field_value(&self.editor.doc.doc, None, field);
                self.start_edit(value);
            }
            Some(Row::Module(name)) => {
                let name = name.clone();
                let field = config::MODULE_FIELDS
                    .get(self.editor.field)
                    .copied()
                    .unwrap_or("symbol");
                let value = field_value(&self.editor.doc.doc, Some(&name), field);
                self.start_edit(value);
            }
            None => {}
        }
    }

    fn start_edit(&mut self, value: String) {
        let cursor = value.chars().count();
        self.editor.editing = Some(Edit {
            buffer: value,
            cursor,
        });
    }

    fn move_editor(&mut self, delta: isize) {
        if self.editor.os_mode {
            nudge(
                &mut self.editor.os_selected,
                self.editor.os_names.len(),
                delta,
            );
            return;
        }
        nudge(&mut self.editor.selected, self.editor.rows.len(), delta);
        self.editor.field = 0;
    }

    fn cycle_field(&mut self, delta: isize) {
        if self.editor.os_mode {
            return;
        }
        let count = self.editor.fields().len();
        if count == 0 {
            return;
        }
        let next = self.editor.field as isize + delta;
        self.editor.field = if next < 0 {
            count - 1
        } else {
            (next as usize) % count
        };
    }

    fn toggle_space(&mut self) {
        if self.editor.os_mode {
            return;
        }
        match self.editor.rows.get(self.editor.selected) {
            Some(Row::Module(name)) => {
                let name = name.clone();
                let disabled = config::is_disabled(&self.editor.doc.doc, &name);
                config::set_disabled(&mut self.editor.doc.doc, &name, !disabled);
            }
            Some(Row::Prompt) => {
                let current = field_value(&self.editor.doc.doc, None, "add_newline");
                let next = if current == "true" { "false" } else { "true" };
                let _ = set_field(&mut self.editor.doc.doc, None, "add_newline", next);
            }
            _ => {}
        }
    }

    fn request_quit(&mut self) {
        if self.editor.dirty() {
            self.confirm_quit = true;
        } else {
            self.quit = true;
        }
    }

    fn open_editor_draft(&mut self) {
        let draft = self.wizard.draft();
        if self.wizard.saved {
            self.editor.open_saved_draft(draft);
        } else {
            self.editor.open_draft(draft, &self.user_text);
        }
        self.screen = Screen::Editor;
    }

    fn save_wizard(&mut self) {
        let draft = self.wizard.draft();
        let text = draft.to_toml();
        match config::write_config(&self.path, &text, true) {
            Ok(backup) => {
                self.wizard.saved = true;
                self.user_text = text;
                self.notice = match backup {
                    Some(path) => format!("Saved. Backup {}", path.display()),
                    None => format!("Saved {}", self.path.display()),
                };
            }
            Err(err) => self.notice = err.to_string(),
        }
    }

    fn save_editor(&mut self) -> bool {
        if !self.editor.dirty() {
            self.notice = "No changes".into();
            return true;
        }
        let text = self.editor.doc.to_toml();
        match config::write_config(&self.path, &text, self.editor.backup_on_save) {
            Ok(backup) => {
                self.editor.mark_saved();
                self.user_text = text;
                self.notice = match backup {
                    Some(path) => format!("Saved. Backup {}", path.display()),
                    None => format!("Saved {}", self.path.display()),
                };
                true
            }
            Err(err) => {
                self.notice = err.to_string();
                false
            }
        }
    }
}

fn nudge(index: &mut usize, len: usize, delta: isize) {
    if len == 0 {
        *index = 0;
        return;
    }
    let next = *index as isize + delta;
    *index = if next < 0 {
        len - 1
    } else if next >= len as isize {
        0
    } else {
        next as usize
    };
}

fn byte_at(text: &str, chars: usize) -> usize {
    text.char_indices()
        .nth(chars)
        .map(|(index, _)| index)
        .unwrap_or(text.len())
}

/// Read-only view the UI uses. Kept next to the mutators so the draw code
/// does not reach into private fields ad hoc.
impl App {
    pub fn wizard_progress(&self) -> (usize, &'static [&'static str]) {
        self.wizard.progress()
    }

    pub fn wizard_question(&self) -> String {
        self.wizard.question(&self.path.display().to_string())
    }

    pub fn wizard_note(&self) -> String {
        self.wizard.note(&self.path.display().to_string())
    }

    pub fn wizard_typing_color(&self) -> bool {
        self.wizard.hex_value().is_some()
    }

    pub fn wizard_keys(&self) -> &'static [(&'static str, &'static str)] {
        self.wizard.keys()
    }

    pub fn wizard_summary(&self) -> Vec<(&'static str, String)> {
        self.wizard.summary()
    }

    pub fn wizard_rows(&self) -> Vec<(String, String, String, bool)> {
        (0..self.wizard.choice_count())
            .map(|index| {
                let (name, meta, detail) = self.wizard.choice(index);
                (name, meta, detail, index == self.wizard.cursor())
            })
            .collect()
    }

    pub fn wizard_color(&self, index: usize) -> Option<((u8, u8, u8), (u8, u8, u8))> {
        self.wizard.choice_rgb(index)
    }

    pub fn editor_keys(&self) -> &'static [(&'static str, &'static str)] {
        if self.editor.filtering {
            &[
                ("type", "filter"),
                ("enter", "keep"),
                ("esc", "show every row"),
            ]
        } else if self.editor.editing.is_some() {
            &[
                ("type", "the value"),
                ("enter", "keep"),
                ("esc", "cancel"),
                ("", "leave empty to remove it"),
            ]
        } else if self.editor.os_mode {
            &[
                ("↑↓", "choose"),
                ("enter", "edit the icon"),
                ("esc", "back"),
            ]
        } else {
            &[
                ("↑↓", "move"),
                ("tab", "field"),
                ("enter", "edit"),
                ("space", "disable"),
                ("/", "filter"),
                ("s", "save"),
                ("q", "quit"),
            ]
        }
    }

    pub fn editor_title(&self) -> String {
        if self.editor.os_mode {
            "OS symbols".into()
        } else if self.editor.filtering {
            format!("/{}", self.editor.filter)
        } else if self.editor.filter.is_empty() {
            "Modules".into()
        } else {
            format!("Modules  /{}", self.editor.filter)
        }
    }

    pub fn editor_rows(&self) -> Vec<(String, bool)> {
        if self.editor.os_mode {
            return self
                .editor
                .os_names
                .iter()
                .enumerate()
                .map(|(index, name)| {
                    let symbol = config::os_symbols(&self.editor.doc.doc)
                        .into_iter()
                        .find(|(os, _)| os == name)
                        .map(|(_, symbol)| symbol)
                        .unwrap_or_default();
                    (
                        format!("{name}  {symbol}"),
                        index == self.editor.os_selected,
                    )
                })
                .collect();
        }
        self.editor
            .rows
            .iter()
            .enumerate()
            .map(|(index, row)| {
                let label = match row {
                    Row::Prompt => "Prompt".to_string(),
                    Row::Os => "OS symbols".to_string(),
                    Row::Module(name) => {
                        let symbol = field_value(&self.editor.doc.doc, Some(name), "symbol");
                        let mark = if config::is_disabled(&self.editor.doc.doc, name) {
                            "off"
                        } else {
                            "on"
                        };
                        if symbol.is_empty() {
                            format!("{name}  {mark}")
                        } else {
                            format!("{name}  {symbol} {mark}")
                        }
                    }
                };
                (label, index == self.editor.selected)
            })
            .collect()
    }

    pub fn editor_fields(&self) -> Vec<(String, String, bool)> {
        if self.editor.os_mode {
            let name = self
                .editor
                .os_names
                .get(self.editor.os_selected)
                .cloned()
                .unwrap_or_default();
            let value = config::os_symbols(&self.editor.doc.doc)
                .into_iter()
                .find(|(os, _)| os == &name)
                .map(|(_, symbol)| symbol)
                .unwrap_or_default();
            return vec![("symbol".into(), self.shown_value(&value, true), true)];
        }
        let module = self.editor.selected_module().map(str::to_string);
        let fields: Vec<&str> = self.editor.fields().to_vec();
        fields
            .into_iter()
            .enumerate()
            .map(|(index, field)| {
                let value = field_value(&self.editor.doc.doc, module.as_deref(), field);
                let selected = index == self.editor.field;
                (
                    field.to_string(),
                    self.shown_value(&value, selected),
                    selected,
                )
            })
            .collect()
    }

    fn shown_value(&self, value: &str, selected: bool) -> String {
        if let Some(edit) = &self.editor.editing {
            if selected {
                return with_cursor(&edit.buffer, edit.cursor);
            }
        }
        if value.is_empty() {
            "default".into()
        } else {
            show_newlines(value)
        }
    }
}

fn with_cursor(buffer: &str, cursor: usize) -> String {
    let visible = show_newlines(buffer);
    let cursor = cursor.min(visible.chars().count());
    let (head, tail) = split_chars(&visible, cursor);
    format!("{head}▏{tail}")
}

fn show_newlines(text: &str) -> String {
    text.replace('\n', "⏎")
}

fn split_chars(text: &str, at: usize) -> (String, String) {
    let byte = byte_at(text, at);
    (text[..byte].to_string(), text[byte..].to_string())
}
