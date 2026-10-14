//! Builds a Starship prompt from shape, pieces, and colors.
//! The current file is edited in place. Presets are not used.

use toml_edit::DocumentMut;

use crate::config::{self, ConfigDoc};

const STEPS: &[&str] = &["Shape", "Lines", "Content", "Colors", "Icons", "Finish"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Plain,
    Powerline,
    Capsules,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lines {
    One,
    Two,
    Spaced,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icons {
    Keep,
    Plain,
    None,
}

#[derive(Clone, Debug)]
pub struct Swatch {
    pub name: String,
    pub hex: String,
}

#[derive(Clone, Debug)]
struct PieceSpec {
    id: &'static str,
    label: &'static str,
    detail: &'static str,
    modules: &'static [(&'static str, &'static str)],
    conditional: bool,
    pinned: bool,
    default_on: bool,
    swatch: &'static str,
}

const PIECES: &[PieceSpec] = &[
    PieceSpec {
        id: "directory",
        label: "Directory",
        detail: "The folder you are in",
        modules: &[("directory", "$path")],
        conditional: false,
        pinned: false,
        default_on: true,
        swatch: "accent",
    },
    PieceSpec {
        id: "git_branch",
        label: "Git branch",
        detail: "The branch name, in a repository",
        modules: &[("git_branch", "$symbol$branch")],
        conditional: false,
        pinned: false,
        default_on: true,
        swatch: "green",
    },
    PieceSpec {
        id: "git_status",
        label: "Git status",
        detail: "Shows when the repository has changes",
        modules: &[("git_status", "$all_status$ahead_behind")],
        conditional: true,
        pinned: false,
        default_on: true,
        swatch: "yellow",
    },
    PieceSpec {
        id: "language",
        label: "Language",
        detail: "Rust, Python, Node, Go, or Bun, when that project is detected",
        modules: &[
            ("rust", "$symbol($version )"),
            ("python", "$symbol($version )"),
            ("nodejs", "$symbol($version )"),
            ("golang", "$symbol($version )"),
            ("bun", "$symbol($version )"),
        ],
        conditional: true,
        pinned: false,
        default_on: false,
        swatch: "blue",
    },
    PieceSpec {
        id: "cmd_duration",
        label: "Command duration",
        detail: "How long the last command took, when it was slow",
        modules: &[("cmd_duration", "$duration")],
        conditional: true,
        pinned: false,
        default_on: false,
        swatch: "yellow",
    },
    PieceSpec {
        id: "status",
        label: "Exit status",
        detail: "The last command's exit code, when it failed",
        modules: &[("status", "$status")],
        conditional: true,
        pinned: false,
        default_on: false,
        swatch: "red",
    },
    PieceSpec {
        id: "time",
        label: "Time",
        detail: "The current time",
        modules: &[("time", "$time")],
        conditional: false,
        pinned: false,
        default_on: false,
        swatch: "magenta",
    },
    PieceSpec {
        id: "username",
        label: "Username",
        detail: "Your user name, including when you are not root",
        modules: &[("username", "$user")],
        conditional: false,
        pinned: false,
        default_on: false,
        swatch: "cyan",
    },
    PieceSpec {
        id: "hostname",
        label: "Hostname",
        detail: "This machine's name, including when you are not on ssh",
        modules: &[("hostname", "$hostname")],
        conditional: false,
        pinned: false,
        default_on: false,
        swatch: "cyan",
    },
    PieceSpec {
        id: "os",
        label: "OS icon",
        detail: "The operating system icon",
        modules: &[("os", "$symbol")],
        conditional: false,
        pinned: false,
        default_on: false,
        swatch: "blue",
    },
    PieceSpec {
        id: "battery",
        label: "Battery",
        detail: "Shows while the laptop is on battery",
        modules: &[("battery", "$symbol$percentage")],
        conditional: true,
        pinned: false,
        default_on: false,
        swatch: "green",
    },
    PieceSpec {
        id: "character",
        label: "Prompt character",
        detail: "Always last, on the last line. It cannot be removed.",
        modules: &[],
        conditional: false,
        pinned: true,
        default_on: true,
        swatch: "accent",
    },
];

#[derive(Clone, Debug)]
pub struct Piece {
    id: &'static str,
    label: &'static str,
    detail: &'static str,
    modules: &'static [(&'static str, &'static str)],
    conditional: bool,
    pinned: bool,
    pub included: bool,
    swatch: usize,
    custom: Option<String>,
    light_text: bool,
    text_chosen: bool,
}

#[derive(Clone, Debug)]
pub struct Design {
    pub shape: Shape,
    pub lines: Lines,
    pub icons: Icons,
    pub pieces: Vec<Piece>,
    pub swatches: Vec<Swatch>,
}

pub struct Catalog {
    pub modules: Vec<String>,
    pub starship_error: Option<String>,
}

impl Catalog {
    pub fn load() -> Self {
        match crate::preview::cli(&["module", "--list"]) {
            Ok(text) => Self {
                modules: parse_names(&text),
                starship_error: None,
            },
            Err(error) => Self {
                modules: fallback_modules(),
                starship_error: Some(error),
            },
        }
    }
}

pub struct Wizard {
    pub step: usize,
    pub defaults: DocumentMut,
    shape_cursor: usize,
    lines_cursor: usize,
    info_cursor: usize,
    color_cursor: usize,
    icons_cursor: usize,
    pub hex: Option<String>,
    design: Design,
    user_text: String,
}

impl Wizard {
    pub fn new(user_text: String, swatches: Vec<Swatch>) -> Self {
        let swatches = if swatches.is_empty() {
            default_swatches()
        } else {
            swatches
        };
        let pieces = PIECES
            .iter()
            .map(|spec| {
                let swatch = swatches
                    .iter()
                    .position(|item| item.name == spec.swatch)
                    .unwrap_or(0);
                let hex = swatches[swatch].hex.clone();
                Piece {
                    id: spec.id,
                    label: spec.label,
                    detail: spec.detail,
                    modules: spec.modules,
                    conditional: spec.conditional,
                    pinned: spec.pinned,
                    included: spec.default_on || spec.pinned,
                    swatch,
                    custom: None,
                    light_text: is_light_bg(&hex),
                    text_chosen: false,
                }
            })
            .collect();
        Self {
            step: 0,
            defaults: DocumentMut::new(),
            shape_cursor: 0,
            lines_cursor: 1,
            info_cursor: 0,
            color_cursor: 0,
            icons_cursor: 0,
            hex: None,
            design: Design {
                shape: Shape::Plain,
                lines: Lines::Two,
                icons: Icons::Keep,
                pieces,
                swatches,
            },
            user_text,
        }
    }

    pub fn progress(&self) -> (usize, &'static [&'static str]) {
        (self.step, STEPS)
    }

    pub fn question(&self, _path: &str) -> String {
        if self.hex.is_some() {
            return "Type a color".into();
        }
        match self.step {
            0 => "How should the pieces sit?".into(),
            1 => "How many lines is the prompt?".into(),
            2 => "What does the prompt show?".into(),
            3 => "What color is each piece?".into(),
            4 => "Do these icons look right?".into(),
            _ => "Adopt this prompt?".into(),
        }
    }

    /// Second line under the question. Empty when the question is enough.

    pub fn summary(&self) -> Vec<(&'static str, String)> {
        let design = self.live_design();
        let shape = match design.shape {
            Shape::Plain => "Plain",
            Shape::Powerline => "Powerline",
            Shape::Capsules => "Capsules",
        };
        let lines = match design.lines {
            Lines::One => "One line",
            Lines::Two => "Two lines",
            Lines::Spaced => "Two lines, with a gap",
        };
        let icons = match design.icons {
            Icons::Keep => "Keep the icons in the file",
            Icons::Plain => "Plain words",
            Icons::None => "No icons",
        };
        let shows = design
            .pieces
            .iter()
            .filter(|piece| piece.included && piece.id != "character")
            .map(|piece| piece.label)
            .collect::<Vec<_>>()
            .join("   ·   ");
        vec![
            ("Shape", shape.into()),
            ("Lines", lines.into()),
            (
                "Shows",
                if shows.is_empty() {
                    "Prompt character".into()
                } else {
                    shows
                },
            ),
            ("Icons", icons.into()),
        ]
    }

    pub fn is_confirm(&self) -> bool {
        self.step == 5
    }

    pub fn choice_count(&self) -> usize {
        match self.step {
            0 | 1 => 3,
            2 => self.design.pieces.len(),
            3 => self.color_indexes().len(),
            4 => 3,
            _ => 0,
        }
    }

    pub fn cursor(&self) -> usize {
        match self.step {
            0 => self.shape_cursor,
            1 => self.lines_cursor,
            2 => self.info_cursor,
            3 => self.color_cursor,
            4 => self.icons_cursor,
            _ => 0,
        }
    }

    /// Name, a short sample or state, and the sentence for the highlighted row.
    pub fn choice(&self, index: usize) -> (String, String, String) {
        match self.step {
            0 => match index {
                0 => (
                    "Plain".into(),
                    "dir  git  ❯".into(),
                    "Pieces sit next to each other. Color is the text.".into(),
                ),
                1 => (
                    "Powerline".into(),
                    "dir \u{e0b0} git \u{e0b0} ❯".into(),
                    "Each piece is a block. A triangle joins one block to the next.".into(),
                ),
                _ => (
                    "Capsules".into(),
                    "\u{e0b6} dir \u{e0b4}  \u{e0b6} git \u{e0b4}  ❯".into(),
                    "Each piece is its own rounded block, with a gap between them.".into(),
                ),
            },
            1 => match index {
                0 => (
                    "One line".into(),
                    "dir git ❯".into(),
                    "Everything, then the prompt character, on a single line.".into(),
                ),
                1 => (
                    "Two lines".into(),
                    "dir git  ⏎  ❯".into(),
                    "The pieces on the first line. The prompt character on the second.".into(),
                ),
                _ => (
                    "Two lines, with a gap".into(),
                    "⏎  dir git  ⏎  ❯".into(),
                    "A blank line, then the pieces, then the prompt character.".into(),
                ),
            },
            2 => {
                let piece = &self.design.pieces[index];
                let state = if piece.pinned {
                    "Always"
                } else if piece.included {
                    "On"
                } else {
                    "Off"
                };
                (piece.label.into(), state.into(), piece.detail.into())
            }
            3 => {
                let piece = &self.design.pieces[self.color_indexes()[index]];
                let mut name = piece
                    .custom
                    .clone()
                    .unwrap_or_else(|| self.design.swatches[piece.swatch].name.clone());
                if piece.text_chosen {
                    let tone = if piece.light_text { "light" } else { "dark" };
                    name = format!("{name}  ·  {tone}");
                }
                (piece.label.into(), name, piece.detail.into())
            }
            4 => match index {
                0 => (
                    "Keep these icons".into(),
                    "\u{f418}   \u{f07b}   \u{f023}".into(),
                    "Use the icons already in your Starship file.".into(),
                ),
                1 => (
                    "Plain words".into(),
                    "git   rust".into(),
                    "Words such as git and rust, if the icons look wrong.".into(),
                ),
                _ => (
                    "No icons".into(),
                    String::new(),
                    "Leave the icons out.".into(),
                ),
            },
            _ => (String::new(), String::new(), String::new()),
        }
    }

    /// Background and foreground for a color-step row.

    pub fn move_cursor(&mut self, delta: isize) {
        let count = self.choice_count();
        if count == 0 {
            return;
        }
        let next = wrap(self.cursor() as isize + delta, count);
        match self.step {
            0 => self.shape_cursor = next,
            1 => self.lines_cursor = next,
            2 => self.info_cursor = next,
            3 => self.color_cursor = next.min(self.color_indexes().len().saturating_sub(1)),
            4 => self.icons_cursor = next,
            _ => {}
        }
    }

    pub fn advance(&mut self) -> bool {
        if self.step >= 5 {
            return false;
        }
        self.commit_cursor();
        self.step += 1;
        if self.step == 3 {
            let len = self.color_indexes().len();
            if self.color_cursor >= len {
                self.color_cursor = 0;
            }
        }
        true
    }

    pub fn back(&mut self) -> bool {
        if self.hex.take().is_some() {
            return true;
        }
        if self.step == 0 {
            return false;
        }
        self.step -= 1;
        true
    }

    pub fn toggle_piece(&mut self) {
        let Some(piece) = self.design.pieces.get_mut(self.info_cursor) else {
            return;
        };
        if piece.pinned {
            return;
        }
        piece.included = !piece.included;
    }

    pub fn move_piece(&mut self, dir: isize) {
        let next = self.info_cursor as isize + dir;
        if next < 0 || next as usize >= self.design.pieces.len() {
            return;
        }
        let next = next as usize;
        if self.design.pieces[self.info_cursor].pinned || self.design.pieces[next].pinned {
            return;
        }
        self.design.pieces.swap(self.info_cursor, next);
        self.info_cursor = next;
    }

    pub fn cycle_color(&mut self, dir: isize) {
        let Some(&index) = self.color_indexes().get(self.color_cursor) else {
            return;
        };
        let count = self.design.swatches.len().max(1);
        let piece = &mut self.design.pieces[index];
        piece.custom = None;
        piece.swatch = wrap(piece.swatch as isize + dir, count);
        if !piece.text_chosen {
            let hex = self.design.swatches[piece.swatch].hex.clone();
            piece.light_text = is_light_bg(&hex);
        }
    }

    pub fn flip_text(&mut self) {
        let Some(&index) = self.color_indexes().get(self.color_cursor) else {
            return;
        };
        let piece = &mut self.design.pieces[index];
        piece.light_text = !piece.light_text;
        piece.text_chosen = true;
    }

    pub fn start_hex(&mut self) {
        self.hex = Some(String::new());
    }

    pub fn hex_char(&mut self, ch: char) {
        if let Some(buffer) = &mut self.hex {
            if buffer.len() < 7 {
                buffer.push(ch);
            }
        }
    }

    pub fn hex_backspace(&mut self) {
        if let Some(buffer) = &mut self.hex {
            buffer.pop();
        }
    }

    pub fn hex_value(&self) -> Option<&str> {
        self.hex.as_deref()
    }

    pub fn commit_hex(&mut self) -> Result<(), String> {
        let Some(raw) = self.hex.clone() else {
            return Ok(());
        };
        let hex = normalize_hex(&raw).ok_or_else(|| "use a color like #cba6f7".to_string())?;
        let Some(&index) = self.color_indexes().get(self.color_cursor) else {
            self.hex = None;
            return Ok(());
        };
        let piece = &mut self.design.pieces[index];
        if !piece.text_chosen {
            piece.light_text = is_light_bg(&hex);
        }
        piece.custom = Some(hex);
        self.hex = None;
        Ok(())
    }

    pub fn preview(&mut self) -> String {
        render_with_defaults(&self.user_text, &self.live_design(), &self.defaults).to_toml()
    }

    pub fn draft(&mut self) -> ConfigDoc {
        self.commit_cursor();
        render_with_defaults(&self.user_text, &self.design, &self.defaults)
    }

    fn commit_cursor(&mut self) {
        match self.step {
            0 => self.design.shape = shape_at(self.shape_cursor),
            1 => self.design.lines = lines_at(self.lines_cursor),
            4 => self.design.icons = icons_at(self.icons_cursor),
            _ => {}
        }
    }

    fn live_design(&self) -> Design {
        let mut design = self.design.clone();
        if self.step == 0 {
            design.shape = shape_at(self.shape_cursor);
        }
        if self.step == 1 {
            design.lines = lines_at(self.lines_cursor);
        }
        if self.step == 4 {
            design.icons = icons_at(self.icons_cursor);
        }
        design
    }

    fn color_indexes(&self) -> Vec<usize> {
        self.design
            .pieces
            .iter()
            .enumerate()
            .filter(|(_, piece)| piece.included)
            .map(|(index, _)| index)
            .collect()
    }
}

#[cfg(test)]
pub fn render(user_text: &str, design: &Design) -> ConfigDoc {
    render_with_defaults(user_text, design, &DocumentMut::new())
}
fn render_with_defaults(user_text: &str, design: &Design, defaults: &DocumentMut) -> ConfigDoc {
    let mut doc = ConfigDoc::parse(user_text);
    if design.icons == Icons::None {
        for piece in design.pieces.iter().filter(|p| p.included) {
            for (module, content) in piece.modules {
                if content.contains("$symbol") && *module != "os" {
                    module_table(&mut doc.doc, module).insert("symbol", toml_edit::value(""));
                }
                if let Some(table) = defaults.get(module).and_then(|i| i.as_table_like()) {
                    for (key, value) in table.iter() {
                        if value.as_str().is_some()
                            && (key == "read_only" || key == "symbol" || key.ends_with("_symbol"))
                        {
                            module_table(&mut doc.doc, module).insert(key, toml_edit::value(""));
                        }
                    }
                }
            }
        }
        module_table(&mut doc.doc, "directory").insert("read_only", toml_edit::value(""));
    }
    apply_icons(&mut doc.doc, design.icons, defaults);
    write_palette(&mut doc.doc, design);
    write_modules(&mut doc.doc, design);
    config::set_format(&mut doc.doc, &format_string(design));
    doc.doc["add_newline"] = toml_edit::value(design.lines == Lines::Spaced);
    if design.lines != Lines::One {
        module_table(&mut doc.doc, "line_break").insert("disabled", toml_edit::value(false));
    }
    doc
}

fn apply_icons(doc: &mut DocumentMut, icons: Icons, defaults: &DocumentMut) {
    if icons != Icons::Keep {
        let mut names = vec![];
        for source in [defaults, &*doc] {
            if let Some(symbols) = source
                .get("os")
                .and_then(|i| i.as_table_like())
                .and_then(|t| t.get("symbols"))
                .and_then(|i| i.as_table_like())
            {
                names.extend(symbols.iter().map(|(k, _)| k.to_string()));
            }
        }
        names.extend(
            [
                "Arch", "Linux", "Macos", "Windows", "Ubuntu", "Debian", "Unknown",
            ]
            .iter()
            .map(|s| s.to_string()),
        );
        names.sort();
        names.dedup();
        let os = module_table(doc, "os");
        let symbols = os.entry("symbols").or_insert_with(toml_edit::table);
        if let Some(table) = symbols.as_table_like_mut() {
            for name in names {
                table.insert(
                    &name,
                    toml_edit::value(if icons == Icons::Plain {
                        format!("{name} ")
                    } else {
                        String::new()
                    }),
                );
            }
        }
    }

    match icons {
        Icons::Keep => {}
        Icons::Plain => {
            for (module, symbol) in PLAIN_SYMBOLS {
                module_table(doc, module).insert("symbol", toml_edit::value(*symbol));
            }
        }
        Icons::None => config::clear_symbols(doc),
    }
    let glyph = match icons {
        Icons::Keep => "❯",
        Icons::Plain | Icons::None => ">",
    };
    let character = module_table(doc, "character");
    for (key, value) in [
        ("success_symbol", format!("[{glyph}](fg:character_fg)")),
        ("error_symbol", format!("[{glyph}](fg:status_fg)")),
        ("vimcmd_symbol", "[❮](fg:character_fg)".into()),
    ] {
        if icons != Icons::Keep || !character.contains_key(key) {
            character.insert(key, toml_edit::value(value));
        }
    }
}

const PLAIN_SYMBOLS: &[(&str, &str)] = &[
    ("git_branch", "git "),
    ("rust", "rust "),
    ("python", "py "),
    ("nodejs", "node "),
    ("golang", "go "),
    ("bun", "bun "),
    ("battery", "bat "),
    ("cmd_duration", "took "),
];

fn write_palette(doc: &mut DocumentMut, design: &Design) {
    doc["palette"] = toml_edit::value("wizard");
    let table = palette_table(doc);
    for piece in design.pieces.iter().filter(|piece| piece.included) {
        let (bg, fg) = piece_colors(piece, &design.swatches, design.shape);
        table.insert(&format!("{}_bg", piece.id), toml_edit::value(bg));
        table.insert(&format!("{}_fg", piece.id), toml_edit::value(fg));
    }
    if !design
        .pieces
        .iter()
        .any(|piece| piece.id == "status" && piece.included)
    {
        table.insert("status_fg", toml_edit::value("#f38ba8"));
    }
}

fn write_modules(doc: &mut DocumentMut, design: &Design) {
    let shown: Vec<&Piece> = design
        .pieces
        .iter()
        .filter(|piece| piece.included)
        .collect();
    for (index, piece) in shown.iter().enumerate() {
        if piece.id == "character" {
            continue;
        }
        let prev = index.checked_sub(1).map(|at| shown[at]);
        let next = shown.get(index + 1).copied();
        let last = piece.modules.len().saturating_sub(1);
        for (offset, (module, content)) in piece.modules.iter().enumerate() {
            let format = module_format(
                piece,
                *content,
                prev,
                next,
                design.shape,
                offset == 0,
                offset == last,
            );
            let style = match design.shape {
                Shape::Plain => format!("fg:{}_fg", piece.id),
                Shape::Powerline | Shape::Capsules => {
                    format!("fg:{0}_fg bg:{0}_bg", piece.id)
                }
            };
            let table = module_table(doc, module);
            table.insert("format", toml_edit::value(format));
            table.insert("style", toml_edit::value(style));
            table.insert("disabled", toml_edit::value(false));
            if *module == "username" {
                table.insert("show_always", toml_edit::value(true));
            }
            if *module == "hostname" {
                table.insert("ssh_only", toml_edit::value(false));
            }
            if *module == "os" {
                table.insert("disabled", toml_edit::value(false));
            }
        }
    }
}

fn format_string(design: &Design) -> String {
    let mut parts = Vec::new();
    for piece in design.pieces.iter().filter(|piece| piece.included) {
        if piece.id == "character" {
            if design.lines != Lines::One {
                parts.push("$line_break".to_string());
            }
            parts.push("$character".to_string());
            continue;
        }
        match design.shape {
            Shape::Plain | Shape::Powerline | Shape::Capsules => {
                for (module, _) in piece.modules {
                    parts.push(format!("${module}"));
                }
            }
        }
    }
    parts.join("")
}

fn module_format(
    piece: &Piece,
    content: &str,
    prev: Option<&Piece>,
    next: Option<&Piece>,
    shape: Shape,
    lead: bool,
    trail: bool,
) -> String {
    match shape {
        Shape::Plain => format!("[{content}]($style) "),
        Shape::Capsules => format!(
            "[](fg:{id}_bg)[ {content} ]($style)[](fg:{id}_bg) ",
            id = piece.id
        ),
        Shape::Powerline => {
            let mut text = String::new();
            if lead && piece.conditional {
                if let Some(prev) = prev {
                    text.push_str(&format!(
                        "[](fg:{prev}_bg bg:{id}_bg)",
                        prev = prev.id,
                        id = piece.id
                    ));
                }
            }
            text.push_str(&format!("[ {content} ]($style)"));
            if !trail {
                return text;
            }
            match next {
                Some(next) if next.id == "character" => {
                    text.push_str(&format!("[](fg:{}_bg)", piece.id));
                }
                Some(next) if !next.conditional => {
                    text.push_str(&format!(
                        "[](fg:{id}_bg bg:{next}_bg)",
                        id = piece.id,
                        next = next.id
                    ));
                }
                Some(_) => {}
                None => text.push_str(&format!("[](fg:{}_bg)", piece.id)),
            }
            text
        }
    }
}

fn piece_colors(piece: &Piece, swatches: &[Swatch], shape: Shape) -> (String, String) {
    let bg = piece.custom.clone().unwrap_or_else(|| {
        swatches
            .get(piece.swatch)
            .map(|s| s.hex.clone())
            .unwrap_or_else(|| "#89b4fa".into())
    });
    let fg = match shape {
        Shape::Plain if !piece.text_chosen => bg.clone(),
        _ if piece.light_text => "#f5f5f5".into(),
        _ => "#1e1e2e".into(),
    };
    (bg, fg)
}

fn shape_at(index: usize) -> Shape {
    match index {
        1 => Shape::Powerline,
        2 => Shape::Capsules,
        _ => Shape::Plain,
    }
}

fn lines_at(index: usize) -> Lines {
    match index {
        0 => Lines::One,
        2 => Lines::Spaced,
        _ => Lines::Two,
    }
}

fn icons_at(index: usize) -> Icons {
    match index {
        1 => Icons::Plain,
        2 => Icons::None,
        _ => Icons::Keep,
    }
}

fn module_table<'a>(doc: &'a mut DocumentMut, module: &str) -> &'a mut toml_edit::Table {
    let item = doc.entry(module).or_insert_with(toml_edit::table);
    if !item.is_table() {
        *item = toml_edit::table();
    }
    item.as_table_mut().expect("module table")
}

fn palette_table(doc: &mut DocumentMut) -> &mut toml_edit::Table {
    let palettes = module_table(doc, "palettes");
    let item = palettes.entry("wizard").or_insert_with(toml_edit::table);
    if !item.is_table() {
        *item = toml_edit::table();
    }
    item.as_table_mut().expect("palette")
}

pub fn default_swatches() -> Vec<Swatch> {
    [
        ("accent", "#B5A0FF"),
        ("blue", "#83D9DC"),
        ("red", "#ED8796"),
        ("green", "#A3D6B0"),
        ("yellow", "#EAC38C"),
        ("cyan", "#94e2d5"),
        ("magenta", "#cba6f7"),
        ("foreground", "#cdd6f4"),
    ]
    .into_iter()
    .map(|(name, hex)| Swatch {
        name: name.into(),
        hex: hex.into(),
    })
    .collect()
}

fn normalize_hex(raw: &str) -> Option<String> {
    let text = raw.trim().trim_start_matches('#');
    let text = match text.len() {
        3 => {
            let chars: Vec<char> = text.chars().collect();
            format!(
                "{}{}{}{}{}{}",
                chars[0], chars[0], chars[1], chars[1], chars[2], chars[2]
            )
        }
        6 => text.to_string(),
        _ => return None,
    };
    if !text.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return None;
    }
    Some(format!("#{}", text.to_lowercase()))
}

fn parse_rgb(hex: &str) -> Option<(u8, u8, u8)> {
    let text = hex.trim().trim_start_matches('#');
    if text.len() != 6 {
        return None;
    }
    let value = u32::from_str_radix(text, 16).ok()?;
    Some(((value >> 16) as u8, (value >> 8) as u8, value as u8))
}

fn is_light_bg(hex: &str) -> bool {
    let Some((r, g, b)) = parse_rgb(hex) else {
        return true;
    };
    let luminance = 0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32;
    luminance < 150.0
}

fn wrap(next: isize, count: usize) -> usize {
    if count == 0 {
        return 0;
    }
    if next < 0 {
        count - 1
    } else if next >= count as isize {
        0
    } else {
        next as usize
    }
}

fn fallback_modules() -> Vec<String> {
    config::ALL_MODULES
        .iter()
        .map(|name| (*name).to_string())
        .collect()
}

fn parse_names(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.contains(' ') && !line.starts_with('-'))
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::field_value;

    fn sample(mut tweak: impl FnMut(&mut Design)) -> ConfigDoc {
        let mut wizard = Wizard::new(String::new(), default_swatches());
        tweak(&mut wizard.design);
        render(
            "# keep me\n\n[git_branch]\nsymbol = \"GB \"\n",
            &wizard.design,
        )
    }

    #[test]
    fn plain_prompt_orders_directory_before_git_and_sets_foreground() {
        let doc = sample(|_| {});
        let format = field_value(&doc.doc, None, "format");
        let dir = format.find("$directory").unwrap();
        let git = format.find("$git_branch").unwrap();
        assert!(dir < git, "{format}");
        let style = field_value(&doc.doc, Some("directory"), "style");
        assert_eq!(style, "fg:directory_fg");
        assert!(
            doc.to_toml().contains("[palettes.wizard]"),
            "{}",
            doc.to_toml()
        );
    }

    #[test]
    fn moving_git_ahead_of_directory_changes_order() {
        let doc = sample(|design| {
            let git = design
                .pieces
                .iter()
                .position(|piece| piece.id == "git_branch")
                .unwrap();
            let dir = design
                .pieces
                .iter()
                .position(|piece| piece.id == "directory")
                .unwrap();
            design.pieces.swap(git, dir);
        });
        let format = field_value(&doc.doc, None, "format");
        assert!(
            format.find("$git_branch").unwrap() < format.find("$directory").unwrap(),
            "{format}"
        );
    }

    #[test]
    fn powerline_prompt_joins_blocks_with_the_palette() {
        let doc = sample(|design| design.shape = Shape::Powerline);
        let saved = doc.to_toml();
        assert!(saved.contains(''), "{saved}");
        let style = field_value(&doc.doc, Some("git_branch"), "style");
        assert!(style.contains("bg:git_branch_bg"), "{style}");
        assert!(saved.contains("git_branch_bg"), "{saved}");
    }

    #[test]
    fn design_keeps_a_comment_and_a_custom_symbol() {
        let doc = sample(|_| {});
        let saved = doc.to_toml();
        assert!(saved.contains("# keep me"), "{saved}");
        assert!(saved.contains("symbol = \"GB \""), "{saved}");
    }
    #[test]
    fn keep_icons_preserves_character_and_os_symbols() {
        let mut w = Wizard::new(
            "[character]\nsuccess_symbol = '[λ](green)'\n[os.symbols]\nArch = 'A'\n".into(),
            default_swatches(),
        );
        let d = w.draft();
        assert_eq!(
            d.doc["character"]["success_symbol"].as_str(),
            Some("[λ](green)")
        );
        assert_eq!(d.doc["os"]["symbols"]["Arch"].as_str(), Some("A"));
    }
    #[test]
    fn no_icons_overrides_defaults_and_two_lines_enable_line_break() {
        let mut w = Wizard::new("[line_break]\ndisabled=true\n".into(), default_swatches());
        w.defaults = "[git_branch]\nsymbol='icon'\n[os.symbols]\nArch='icon'\n"
            .parse()
            .unwrap();
        w.design.icons = Icons::None;
        let d = w.draft();
        assert_eq!(d.doc["git_branch"]["symbol"].as_str(), Some(""));
        assert_eq!(d.doc["os"]["symbols"]["Arch"].as_str(), Some(""));
        assert_eq!(d.doc["line_break"]["disabled"].as_bool(), Some(false));
    }
}
