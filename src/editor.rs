//! Plain-language controls and lossless boundaries for the everyday editor.
use crate::{
    app::{lookup, App},
    config,
};
use toml_edit::{DocumentMut, Item};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Group(usize),
    WholePrompt,
    Add,
    Insert(String),
    InsertFamily(usize),
    EnableAll,
    Remove,
    Expand,
    Builder,
    Shift(isize),
    Side,
    Toggle(Vec<String>),
    Text(Vec<String>),
    Number(Vec<String>),
    Color(bool),
    SetColor(bool, String),
    Palette,
    SetPalette(usize),
    Advanced,
}
#[derive(Clone)]
pub struct Control {
    pub label: String,
    pub help: String,
    pub action: Action,
}
pub struct Family {
    pub name: &'static str,
    pub detail: &'static str,
    pub members: &'static [&'static str],
}
pub const FAMILIES: &[Family] = &[
    Family {
        name: "Programming languages",
        detail: "Detected language versions",
        members: &[
            "bun",
            "c",
            "cobol",
            "cpp",
            "daml",
            "dart",
            "deno",
            "dotnet",
            "elixir",
            "elm",
            "erlang",
            "fennel",
            "fortran",
            "gleam",
            "golang",
            "haskell",
            "haxe",
            "java",
            "julia",
            "kotlin",
            "lua",
            "mojo",
            "nim",
            "nodejs",
            "ocaml",
            "odin",
            "opa",
            "perl",
            "php",
            "purescript",
            "python",
            "quarto",
            "raku",
            "rlang",
            "red",
            "ruby",
            "rust",
            "scala",
            "solidity",
            "swift",
            "typst",
            "vlang",
            "zig",
            "crystal",
        ],
    },
    Family {
        name: "Git",
        detail: "Branch, changes and commits",
        members: &[
            "git_branch",
            "git_commit",
            "git_state",
            "git_metrics",
            "git_status",
        ],
    },
    Family {
        name: "Other repositories",
        detail: "Mercurial, Fossil and Pijul",
        members: &[
            "vcsh",
            "fossil_branch",
            "fossil_metrics",
            "hg_branch",
            "hg_state",
            "pijul_channel",
        ],
    },
    Family {
        name: "Cloud services",
        detail: "Active profiles and contexts",
        members: &["aws", "gcloud", "azure", "openstack", "kubernetes", "nats"],
    },
    Family {
        name: "Containers",
        detail: "Docker and container contexts",
        members: &["docker_context", "container", "singularity"],
    },
    Family {
        name: "Development environments",
        detail: "Active shells and environments",
        members: &[
            "guix_shell",
            "nix_shell",
            "conda",
            "pixi",
            "spack",
            "direnv",
            "mise",
        ],
    },
    Family {
        name: "Build and deployment tools",
        detail: "Build versions and workspaces",
        members: &[
            "cmake",
            "gradle",
            "helm",
            "maven",
            "terraform",
            "pulumi",
            "meson",
            "vagrant",
            "xmake",
            "buf",
        ],
    },
    Family {
        name: "System information",
        detail: "User, host, OS and resources",
        members: &[
            "username",
            "hostname",
            "localip",
            "shlvl",
            "memory_usage",
            "battery",
            "netns",
            "os",
            "shell",
            "sudo",
        ],
    },
    Family {
        name: "Custom commands",
        detail: "Your configured command output",
        members: &[],
    },
    Family {
        name: "Environment variables",
        detail: "Your configured environment values",
        members: &[],
    },
];
pub fn family(module: &str) -> Option<usize> {
    if module.starts_with("custom.") {
        return Some(8);
    }
    if module == "env_var" || module.starts_with("env_var.") {
        return Some(9);
    }
    FAMILIES
        .iter()
        .position(|group| group.members.contains(&module))
}
pub const GROUPS: &[&str] = &["Layout", "Colors", "Symbols", "Details"];
pub fn name(id: &str) -> String {
    match id {
        "all" => "Automatic Starship content",
        "directory" => "Current folder",
        "git_branch" => "Git branch",
        "git_status" => "Git changes",
        "git_commit" => "Git commit",
        "git_state" => "Git operation",
        "cmd_duration" => "Command duration",
        "character" => "Prompt symbol",
        "line_break" => "New line",
        "time" => "Clock",
        "jobs" => "Background jobs",
        "username" => "User name",
        "hostname" => "Computer name",
        "os" => "Operating system",
        "status" => "Command exit status",
        "battery" => "Battery",
        "fill" => "Flexible spacing",
        "nodejs" => "Node.js",
        "python" => "Python",
        "rust" => "Rust",
        "golang" => "Go",
        "kubernetes" => "Kubernetes context",
        "docker_context" => "Docker context",
        _ => return id.replace('_', " "),
    }
    .into()
}
pub fn part_label(id: &str) -> String {
    let detail = match id {
        "directory" => "Working folder",
        "git_branch" | "hg_branch" | "fossil_branch" | "pijul_channel" => "Current branch",
        "git_status" | "git_metrics" | "hg_state" | "fossil_metrics" => "Repository changes",
        "git_commit" => "Commit ID",
        "git_state" => "Merge or rebase state",
        "character" => "Command marker",
        "cmd_duration" => "Command time",
        "time" => "Current time",
        "jobs" => "Background commands",
        "line_break" => "Start another line",
        "fill" => "Push content right",
        "os" => "System icon",
        "username" => "Current user",
        "hostname" => "Computer name",
        "status" => "Last exit code",
        "battery" => "Charge level",
        "localip" => "Local IP address",
        "shlvl" => "Nested shell depth",
        "shell" => "Current shell",
        "memory_usage" => "Memory in use",
        "package" => "Project version",
        "sudo" => "Sudo credentials active",
        "aws" => "AWS profile and region",
        "gcloud" => "Google Cloud project",
        "azure" => "Azure subscription",
        "openstack" => "OpenStack cloud",
        "kubernetes" => "Cluster and namespace",
        "docker_context" => "Docker context",
        "container" | "singularity" => "Container name",
        "netns" => "Network namespace",
        "nix_shell" | "guix_shell" => "Development shell",
        "conda" | "pixi" | "spack" | "meson" => "Active environment",
        "direnv" => "Directory environment",
        "terraform" => "Terraform workspace",
        "pulumi" => "Pulumi stack",
        "vagrant" => "Vagrant version",
        "vcsh" => "Vcsh repository",
        "mise" => "Active tool versions",
        "env_var" => "Environment value",
        "custom" => "Custom command output",
        _ if id.starts_with("custom.") => "Custom command output",
        _ if id.starts_with("env_var.") => "Environment value",
        _ if config::ALL_MODULES.contains(&id) => "Detected tool version",
        _ => "Detected environment",
    };
    format!("{} · {detail}", name(id))
}
pub fn description(id: &str) -> &'static str {
    match id {
        "all" => "Starship automatically chooses its usual content. Choose individual parts to customize the order.",
        "directory" => "The folder you are working in.",
        "git_branch" => "Your current branch; appears inside a Git repository.",
        "git_status" => "Changes, staged files, and commits to push; appears when Git has something to report.",
        "character" => "The marker beside the place where you type commands.",
        "cmd_duration" => "How long the previous command took; appears after the chosen time threshold.",
        "time" => "The current clock time.", "jobs" => "Commands still running in the background.",
        "line_break" => "Starts a new prompt line.", "fill" => "Uses available space to push later content to the right.",
        "os" => "An icon or name for your operating system.",
        "username" => "The current user; usually appears for remote or privileged sessions.",
        "hostname" => "The computer name; usually appears in remote sessions.",
        "status" => "The exit status of the previous command; usually hidden after success.",
        "battery" => "Battery level; appears when a battery is detected and its display threshold is met.",
        _ => "Shown when Starship detects the relevant project or environment. Advanced settings expose its full configuration.",
    }
}
pub fn path(module: &str, key: &str) -> Vec<String> {
    module
        .split('.')
        .chain(std::iter::once(key))
        .map(str::to_string)
        .collect()
}
pub fn value(a: &App, keys: &[String]) -> Option<Item> {
    lookup(&a.workspace.doc, keys)
        .or_else(|| lookup(&a.defaults, keys))
        .cloned()
}
pub fn controls(a: &App) -> Vec<Control> {
    let mut controls = vec![];
    for group in [0, 1, 2, 3] {
        controls.extend(group_controls(a, group));
    }
    if a.prompt_settings {
        controls.retain(|c| {
            matches!(c.action,Action::Toggle(ref p) if p==&["add_newline".to_string()])
                || matches!(c.action, Action::Builder | Action::Palette)
        });
    } else {
        controls.retain(|c| {
            !matches!(
                c.action,
                Action::Builder
                    | Action::Palette
                    | Action::Advanced
                    | Action::Shift(_)
                    | Action::Side
                    | Action::Remove
            ) && !matches!(c.action,Action::Toggle(ref p) if p==&["add_newline".to_string()])
        });
        // Everyday values first; structural actions stay at the bottom of the same list.
        let structural = group_controls(a, 0)
            .into_iter()
            .filter(|c| matches!(c.action, Action::Shift(_) | Action::Side | Action::Remove));
        controls.extend(structural);
    }
    controls
}
fn group_controls(a: &App, group: usize) -> Vec<Control> {
    let mut out = vec![];
    let mut add = |label: String, help: &str, action| {
        out.push(Control {
            label,
            help: help.into(),
            action,
        })
    };
    let m = a.module();
    match group {
        0 => {
            let blank = a
                .workspace
                .doc
                .get("add_newline")
                .and_then(Item::as_bool)
                .unwrap_or(true);
            add(
                format!(
                    "Blank line above prompt: {}",
                    if blank { "On" } else { "Off" }
                ),
                "Adds breathing room between commands. Click or press Enter to switch it.",
                Action::Toggle(vec!["add_newline".into()]),
            );
            add("Change shape or line layout…".into(), "Opens a guided replacement design. Your current draft stays intact until you finish and confirm.", Action::Builder);
            if let Some(m) = &m {
                if m == "all" {
                    add("Choose individual parts…".into(), "Expands automatic content into Starship’s installed module order, preserving other text and settings.", Action::Expand);
                } else {
                    let enabled = a.enabled(m);
                    add(format!("Show {}: {}",name(m),if enabled {"On"} else {"Off"}), "Allows this part to appear when its conditions are met. Turning it on does not force it to appear in every directory.",Action::Toggle(path(m,"disabled")));
                    add("Move earlier".into(), "Moves this part before the previous part when their format boundaries can be safely separated.",Action::Shift(-1));
                    add("Move later".into(), "Moves this part after the next part when their format boundaries can be safely separated.",Action::Shift(1));
                    add("Move to other side".into(), "Moves this part between the left and right prompt when its format can be safely separated.",Action::Side);
                    add("Remove from prompt".into(), "Removes this part’s reference without deleting its settings. Undo restores it.",Action::Remove);
                }
            }
        }
        1 => {
            add("Prompt color theme…".into(), "Choose terminal colors or a named palette for the prompt. This does not change Aster’s appearance.", Action::Palette);
            if let Some(m) = m.filter(|m| !["all", "line_break"].contains(&m.as_str())) {
                let style = if m == "character" {
                    value(a, &path(&m, "success_symbol"))
                        .and_then(|i| i.as_str().and_then(symbol_style).map(str::to_string))
                        .unwrap_or_default()
                } else {
                    value(a, &path(&m, "style"))
                        .and_then(|i| i.as_str().map(str::to_string))
                        .unwrap_or_default()
                };
                add(format!("Text color: {}",style_color(&style,false)),"Choose a terminal color or enter a hex color. Enclosing styled groups may share this color with nearby decorations.",Action::Color(false));
                add(format!("Background color: {}",style_color(&style,true)),"Choose the background behind this part. Default lets the terminal background show through.",Action::Color(true));
            }
        }
        2 => {
            if let Some(m) = m.filter(|m| m != "all") {
                let fields: &[(&str, &str, &str)] = if m == "character" {
                    &[("success_symbol","After successful command","The prompt marker after a command succeeds. Enter the symbol itself; no quotes needed."),("error_symbol","After failed command","The prompt marker after a command fails. Enter the symbol itself; no quotes needed."),("vimcmd_symbol","Vi command mode","The marker when your shell is in vi command mode.")]
                } else if m == "directory" {
                    &[(
                        "read_only",
                        "Read-only marker",
                        "The marker shown when the current folder cannot be written to.",
                    )]
                } else {
                    &[("symbol","Icon or symbol","The icon displayed before this part. An empty field removes the icon; Nerd Font icons need a compatible font.")]
                };
                for (key, label, help) in fields {
                    // Only offer known symbol fields; custom modules may still define one.
                    if m != "character" && value(a, &path(&m, key)).is_none() {
                        continue;
                    }
                    let val = value(a, &path(&m, key))
                        .and_then(|i| i.as_str().map(str::to_string))
                        .unwrap_or_default();
                    add(
                        format!("{label}: {}", plain_symbol(&val)),
                        help,
                        Action::Text(path(&m, key)),
                    );
                }
            }
        }
        _ => {
            if let Some(m) = m.filter(|m| m != "all") {
                let fields: &[(&str,&str,&str)] = match m.as_str() {
                    "directory"=> &[("truncation_length","Folders to show","The maximum number of folder components displayed. Zero shows the full path."),("truncate_to_repo","Shorten to repository","Starts the displayed path at the Git repository root.")],
                    "cmd_duration"=> &[("min_time","Show after (milliseconds)","Displays duration only when the command takes at least this long. 2000 milliseconds is two seconds.")],
                    "time"=> &[("time_format","Clock format","Use %H:%M for 24-hour time, or %I:%M %p for 12-hour time.")],
                    "username"=> &[("show_always","Always show user","Shows your user name in local sessions as well as remote or privileged sessions.")],
                    "hostname"=> &[("ssh_only","Only in remote sessions","Shows the computer name only when connected over SSH.")],
                    _=> &[],
                };
                for (key, label, help) in fields {
                    let keys = path(&m, key);
                    let v = value(a, &keys);
                    let (display, action) = match v {
                        Some(i) if i.as_bool().is_some() => (
                            if i.as_bool() == Some(true) {
                                "On"
                            } else {
                                "Off"
                            }
                            .into(),
                            Action::Toggle(keys),
                        ),
                        Some(i) if i.as_integer().is_some() => {
                            (i.as_integer().unwrap().to_string(), Action::Number(keys))
                        }
                        Some(i) if i.as_str().is_some() => {
                            (i.as_str().unwrap().to_string(), Action::Text(keys))
                        }
                        _ => continue,
                    };
                    add(format!("{label}: {display}"), help, action);
                }
            }
        }
    }
    add("Advanced settings…".into(),"Opens all technical properties for this part. Raw formats and detection rules are kept here.",Action::Advanced);
    out
}
pub fn style_color(style: &str, bg: bool) -> String {
    style
        .split_whitespace()
        .find_map(|s| {
            if bg {
                s.strip_prefix("bg:")
            } else if let Some(c) = s.strip_prefix("fg:") {
                Some(c)
            } else if !s.contains(':')
                && ![
                    "bold",
                    "dimmed",
                    "italic",
                    "underline",
                    "blink",
                    "reversed",
                    "hidden",
                    "strikethrough",
                    "none",
                ]
                .contains(&s)
            {
                Some(s)
            } else {
                None
            }
        })
        .unwrap_or("default")
        .into()
}
pub fn recolor(style: &str, bg: bool, color: &str) -> String {
    let mut pieces = style
        .split_whitespace()
        .filter(|s| {
            if bg {
                !s.starts_with("bg:")
            } else {
                s.starts_with("bg:")
                    || [
                        "bold",
                        "dimmed",
                        "italic",
                        "underline",
                        "blink",
                        "reversed",
                        "hidden",
                        "strikethrough",
                        "none",
                    ]
                    .contains(s)
            }
        })
        .map(str::to_string)
        .collect::<Vec<_>>();
    if color != "default" {
        pieces.push(format!("{}:{color}", if bg { "bg" } else { "fg" }));
    }
    pieces.join(" ")
}
pub fn symbol_style(s: &str) -> Option<&str> {
    s.rfind("](")
        .filter(|_| s.starts_with('[') && s.ends_with(')'))
        .map(|i| &s[i + 2..s.len() - 1])
}
pub fn color_symbol(s: &str, bg: bool, color: &str) -> String {
    if let Some(style) = symbol_style(s) {
        let text = plain_symbol(s);
        let style = recolor(style, bg, color);
        if style.is_empty() {
            text
        } else {
            format!("[{text}]({style})")
        }
    } else if color == "default" {
        s.into()
    } else {
        format!("[{s}]({}:{color})", if bg { "bg" } else { "fg" })
    }
}
pub fn plain_symbol(s: &str) -> String {
    if s.starts_with('[') {
        if let Some(end) = s.rfind("](") {
            return s[1..end].into();
        }
    }
    s.into()
}
pub fn replace_symbol(old: &str, text: &str) -> String {
    if old.starts_with('[') {
        if let Some(end) = old.rfind("](") {
            return format!("[{text}{}", &old[end..]);
        }
    }
    text.into()
}

// A unit is an entire top-level styled/conditional group, variable or literal.
// Structural edits only operate on whole units, never reconstruct their syntax.
#[derive(Clone, Debug)]
pub struct Unit {
    pub start: usize,
    pub end: usize,
    pub modules: Vec<String>,
}
pub fn units(format: &str) -> Result<Vec<Unit>, String> {
    let b = format.as_bytes();
    let mut i = 0;
    let mut out = vec![];
    while i < b.len() {
        let start = i;
        if b[i] == b'\\' {
            i = if i + 1 < b.len() {
                i + 1 + format[i + 1..].chars().next().unwrap().len_utf8()
            } else {
                b.len()
            };
        } else if b[i] == b'[' || b[i] == b'(' {
            let open = b[i];
            let close = if open == b'[' { b']' } else { b')' };
            let mut stack = vec![close];
            i += 1;
            while i < b.len() && !stack.is_empty() {
                if b[i] == b'\\' {
                    i = if i + 1 < b.len() {
                        i + 1 + format[i + 1..].chars().next().unwrap().len_utf8()
                    } else {
                        b.len()
                    };
                    continue;
                }
                match b[i] {
                    b'[' => stack.push(b']'),
                    b'(' => stack.push(b')'),
                    b']' | b')' => {
                        if stack.pop() != Some(b[i]) {
                            return Err(
                                "This format has unmatched groups. Edit it under Advanced.".into(),
                            );
                        }
                    }
                    _ => {}
                }
                i += 1;
            }
            if !stack.is_empty() {
                return Err("This format has unmatched groups. Edit it under Advanced.".into());
            }
            if open == b'[' && i < b.len() && b[i] == b'(' {
                i += 1;
                while i < b.len() && b[i] != b')' {
                    if b[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
                if i >= b.len() {
                    return Err("This styled group has no closing parenthesis.".into());
                }
                i += 1;
            }
        } else if b[i] == b'$' {
            i += 1;
            if i < b.len() && b[i] == b'{' {
                i += 1;
                while i < b.len() && b[i] != b'}' {
                    i += 1;
                }
                if i == b.len() {
                    return Err("This variable has no closing brace.".into());
                }
                i += 1;
            } else {
                while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'.')
                {
                    i += 1;
                }
            }
        } else {
            i += 1;
            while i < b.len() && ![b'$', b'[', b'(', b'\\'].contains(&b[i]) {
                i += 1;
            }
        }
        let chunk = &format[start..i];
        out.push(Unit {
            start,
            end: i,
            modules: variables(chunk),
        });
    }
    Ok(out)
}
pub fn variables(format: &str) -> Vec<String> {
    // Ignore style annotations: `$palette` inside a style is not a prompt part.
    let mut names = vec![];
    let b = format.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\\' {
            i = if i + 1 < b.len() {
                i + 1 + format[i + 1..].chars().next().unwrap().len_utf8()
            } else {
                b.len()
            };
            continue;
        }
        if b[i] == b']' && b.get(i + 1) == Some(&b'(') {
            i += 2;
            while i < b.len() && b[i] != b')' {
                i += 1;
            }
            continue;
        }
        if b[i] != b'$' {
            i += 1;
            continue;
        }
        i += 1;
        let brace = b.get(i) == Some(&b'{');
        if brace {
            i += 1;
        }
        let start = i;
        if brace {
            while i < b.len() && b[i] != b'}' {
                i += 1;
            }
        } else {
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'.') {
                i += 1;
            }
        }
        if i > start {
            names.push(format[start..i].to_string());
        }
        if brace && b.get(i) == Some(&b'}') {
            i += 1;
        }
    }
    names
}
pub fn format_color(format: &str, module: &str, bg: bool, color: &str) -> String {
    // Update containing styled groups from right to left to preserve byte ranges.
    let b = format.as_bytes();
    let mut stack = vec![];
    let mut patches = vec![];
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\\' {
            i = if i + 1 < b.len() {
                i + 1 + format[i + 1..].chars().next().unwrap().len_utf8()
            } else {
                b.len()
            };
            continue;
        }
        if b[i] == b'[' {
            stack.push(i);
        }
        if b[i] == b']' {
            if let Some(start) = stack.pop() {
                if b.get(i + 1) == Some(&b'(') {
                    let end = format[i + 2..].find(')').map(|n| i + 2 + n);
                    if let Some(end) = end {
                        if !config::find_vars(&format[start + 1..i], module).is_empty() {
                            patches.push((i + 2, end, recolor(&format[i + 2..end], bg, color)));
                        }
                        i = end;
                    }
                }
            }
        }
        i += 1;
    }
    patches.sort_by_key(|p| p.0);
    let mut result = format.to_string();
    for (start, end, text) in patches.into_iter().rev() {
        result.replace_range(start..end, &text);
    }
    result
}
pub const PALETTES: &[(&str, [&str; 5])] = &[
    (
        "Terminal colors",
        ["purple", "cyan", "green", "red", "black"],
    ),
    (
        "Aurora",
        ["#b4a6ed", "#7dcfff", "#a6e3c1", "#e78284", "#151522"],
    ),
    (
        "Obsidian",
        ["#a4afbd", "#778899", "#c9d1d9", "#e78284", "#151522"],
    ),
    (
        "Deep Forest",
        ["#9fbc85", "#70aaa0", "#ded6af", "#e78284", "#151522"],
    ),
    (
        "Ember",
        ["#edb86e", "#cd8b62", "#f4d9b0", "#e78284", "#151522"],
    ),
];
pub fn apply_palette(doc: &mut DocumentMut, palette: usize, modules: &[String]) {
    let colors = PALETTES[palette.min(PALETTES.len() - 1)].1;
    // Replace color tokens only inside styles; literal prompt text is untouched.
    fn map_style(style: &str, c: [&str; 5]) -> String {
        style
            .split_whitespace()
            .map(|token| {
                let (prefix, col) = if let Some(v) = token.strip_prefix("fg:") {
                    ("fg:", v)
                } else if let Some(v) = token.strip_prefix("bg:") {
                    ("bg:", v)
                } else {
                    ("", token)
                };
                let role = match col {
                    "purple" | "magenta" | "bright-purple" | "#b4a6ed" | "#a4afbd" | "#9fbc85"
                    | "#edb86e" => Some(0),
                    "cyan" | "blue" | "bright-cyan" | "#7dcfff" | "#778899" | "#70aaa0"
                    | "#cd8b62" => Some(1),
                    "green" | "yellow" | "bright-green" | "#a6e3c1" | "#c9d1d9" | "#ded6af"
                    | "#f4d9b0" => Some(2),
                    "red" | "bright-red" | "#e78284" => Some(3),
                    "black" | "#151522" => Some(4),
                    _ if col.starts_with('#') => Some(if prefix == "bg:" { 4 } else { 0 }),
                    _ => None,
                };
                role.map(|r| format!("{prefix}{}", c[r]))
                    .unwrap_or_else(|| token.into())
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
    fn markup(text: &str, c: [&str; 5]) -> String {
        let mut out = String::new();
        let mut pos = 0;
        while let Some(start) = text[pos..].find("](") {
            let start = pos + start + 2;
            let Some(end) = text[start..].find(')').map(|n| start + n) else {
                break;
            };
            out.push_str(&text[pos..start]);
            out.push_str(&map_style(&text[start..end], c));
            out.push(')');
            pos = end + 1;
        }
        out.push_str(&text[pos..]);
        out
    }
    fn walk(table: &mut dyn toml_edit::TableLike, c: [&str; 5]) {
        for (key, item) in table.iter_mut() {
            if ["palettes", "custom", "env_var", "profiles"].contains(&key.get()) {
                continue;
            }
            if let Some(t) = item.as_table_like_mut() {
                walk(t, c);
            } else if let Some(s) = item.as_str() {
                let key = key.get();
                if key == "style" || key.ends_with("_style") {
                    *item = toml_edit::value(map_style(s, c));
                } else if key == "format"
                    || key == "right_format"
                    || key == "symbol"
                    || key.ends_with("_symbol")
                {
                    *item = toml_edit::value(markup(s, c));
                }
            }
        }
    }
    for (key, item) in doc.as_table_mut().iter_mut() {
        let key = key.get();
        if config::ALL_MODULES.contains(&key) && !["custom", "env_var"].contains(&key) {
            if let Some(table) = item.as_table_like_mut() {
                walk(table, colors);
            }
        } else if ["format", "right_format", "continuation_prompt"].contains(&key) {
            if let Some(s) = item.as_str() {
                *item = toml_edit::value(markup(s, colors));
            }
        }
    }
    for m in modules {
        if ["all", "line_break", "fill", "character"].contains(&m.as_str()) {
            continue;
        }
        let role = if m == "character" || m == "directory" {
            0
        } else if m.starts_with("git_") || m == "time" {
            1
        } else {
            2
        };
        let keys = path(m, "style");
        let style = lookup(doc, &keys).and_then(Item::as_str).unwrap_or("");
        crate::app::set(
            doc,
            &keys,
            toml_edit::value(recolor(style, false, colors[role])),
        );
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn simple_editor_lossless_styled_parts_and_colors() {
        let s = "[╭─ $directory](bold purple)($git_branch$git_status)\\$literal\n$character";
        assert_eq!(variables("${custom.界}\\$ignored"), vec!["custom.界"]);
        assert!(units("[broken").is_err());
        assert!(units("\\✦$directory").is_ok());
        let u = units(s).unwrap();
        assert_eq!(u.iter().map(|u| &s[u.start..u.end]).collect::<String>(), s);
        assert_eq!(
            variables(s),
            vec!["directory", "git_branch", "git_status", "character"]
        );
        assert_eq!(
            format_color(s, "directory", false, "cyan"),
            s.replace("bold purple", "bold fg:cyan")
        );
        assert_eq!(replace_symbol("[❯](bold green)", "→"), "[→](bold green)");
    }
}

pub fn property_help(key: &str) -> &'static str {
    match key.rsplit('.').next().unwrap_or(key) {
        "disabled"=>"true turns this part off; false allows it to appear when detected.",
        "style"=>"Text and background styling. Example: bold cyan bg:black.",
        "format"|"right_format"=>"A Starship expression controlling content and formatting. Preserve variable references and balanced groups.",
        "symbol"|"success_symbol"|"error_symbol"|"vimcmd_symbol"=>"An icon or prompt marker. Nerd Font icons require a compatible font.",
        "detect_extensions"=>"Show this part when the folder contains a file with one of these extensions.",
        "detect_files"=>"Show this part when one of these file names is present.",
        "detect_folders"=>"Show this part when one of these folder names is present.",
        "command"=>"A custom shell command whose output becomes prompt content. Preview requires session approval.",
        "when"=>"A shell command used as a visibility condition. Preview requires session approval.",
        "min_time"=>"Minimum command duration in milliseconds before this part is displayed.",
        "truncation_length"=>"Maximum folder components to show; zero keeps the full path.",
        "truncate_to_repo"=>"Start the displayed path at the repository root.",
        "add_newline"=>"Add a blank line before the prompt.",
        "time_format"=>"Clock format: %H:%M is 24-hour time; %I:%M %p is 12-hour time.",
        "scan_timeout"|"command_timeout"=>"Maximum time in milliseconds for scanning or commands before Starship stops waiting.",
        "show_always"=>"Display even when the usual visibility condition would hide this part.",
        "ssh_only"=>"Display only in a remote SSH session.",
        _=>"Undocumented setting: no built-in explanation is available. Its technical key and TOML value are preserved.",
    }
}
