//! Round-trip edits of a Starship config. `toml_edit` keeps comments,
//! key order, and every key this screen does not touch.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Context;
use toml_edit::{DocumentMut, Item, Table};

/// `$all` in the order `starship print-config` documents for this install.
pub const ALL_MODULES: &[&str] = &[
    "username",
    "hostname",
    "localip",
    "shlvl",
    "singularity",
    "kubernetes",
    "nats",
    "directory",
    "vcsh",
    "fossil_branch",
    "fossil_metrics",
    "git_branch",
    "git_commit",
    "git_state",
    "git_metrics",
    "git_status",
    "hg_branch",
    "hg_state",
    "pijul_channel",
    "docker_context",
    "package",
    "bun",
    "c",
    "cmake",
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
    "gradle",
    "haskell",
    "haxe",
    "helm",
    "java",
    "julia",
    "kotlin",
    "lua",
    "maven",
    "mojo",
    "nim",
    "nodejs",
    "ocaml",
    "odin",
    "opa",
    "perl",
    "php",
    "pulumi",
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
    "terraform",
    "typst",
    "vlang",
    "vagrant",
    "xmake",
    "zig",
    "buf",
    "guix_shell",
    "nix_shell",
    "conda",
    "pixi",
    "meson",
    "spack",
    "memory_usage",
    "aws",
    "gcloud",
    "openstack",
    "azure",
    "direnv",
    "env_var",
    "mise",
    "crystal",
    "custom",
    "sudo",
    "cmd_duration",
    "line_break",
    "jobs",
    "battery",
    "time",
    "status",
    "container",
    "netns",
    "os",
    "shell",
    "character",
];

/// Modules the "Language version" toggle adds or removes.
pub const LANGS: &[&str] = &[
    "bun",
    "c",
    "cmake",
    "cobol",
    "cpp",
    "crystal",
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
];

pub const PROMPT_FIELDS: &[&str] = &[
    "format",
    "right_format",
    "add_newline",
    "scan_timeout",
    "command_timeout",
];

pub const MODULE_FIELDS: &[&str] = &["symbol", "format", "style", "disabled"];

#[derive(Clone, Debug)]
pub struct ConfigDoc {
    pub doc: DocumentMut,
}

impl ConfigDoc {
    pub fn parse(text: &str) -> Self {
        let doc = text
            .parse::<DocumentMut>()
            .unwrap_or_else(|_| DocumentMut::new());
        Self { doc }
    }

    pub fn to_toml(&self) -> String {
        self.doc.to_string()
    }
}

pub fn config_path() -> PathBuf {
    if let Some(path) = std::env::var_os("STARSHIP_CONFIG") {
        if !path.is_empty() {
            return PathBuf::from(path);
        }
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config/starship.toml")
}

pub fn read_config(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

/// Copy the current file beside itself, then write. The copy is
/// `name.bak.<unix seconds>`.
pub fn write_config(path: &Path, text: &str, backup: bool) -> anyhow::Result<Option<PathBuf>> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .with_context(|| format!("could not create {}", parent.display()))?;
        }
    }
    let mut copied = None;
    if backup && path.exists() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("starship.toml");
        let dest = path.with_file_name(format!("{name}.bak.{stamp}"));
        fs::copy(path, &dest).with_context(|| format!("could not copy {}", path.display()))?;
        copied = Some(dest);
    }
    fs::write(path, text).with_context(|| format!("could not write {}", path.display()))?;
    Ok(copied)
}

pub fn expand_all() -> String {
    ALL_MODULES.iter().map(|name| format!("${name}")).collect()
}

pub fn is_all(format: &str) -> bool {
    format.trim() == "$all"
}

pub fn format_of(doc: &DocumentMut) -> Option<String> {
    doc.get("format").and_then(Item::as_str).map(str::to_string)
}

pub fn logical_format(doc: &DocumentMut) -> String {
    match format_of(doc) {
        Some(text) if !text.trim().is_empty() => text,
        _ => "$all".to_string(),
    }
}

pub fn set_format(doc: &mut DocumentMut, format: &str) {
    doc["format"] = toml_edit::value(format);
}

pub fn is_disabled(doc: &DocumentMut, module: &str) -> bool {
    doc.get(module)
        .and_then(Item::as_table)
        .and_then(|table| table.get("disabled"))
        .and_then(Item::as_bool)
        .unwrap_or(false)
}

pub fn set_disabled(doc: &mut DocumentMut, module: &str, disabled: bool) {
    if disabled {
        module_table(doc, module).insert("disabled", toml_edit::value(true));
    } else if let Some(table) = doc.get_mut(module).and_then(Item::as_table_mut) {
        table.remove("disabled");
    }
}

pub fn module_shown(doc: &DocumentMut, name: &str) -> bool {
    if is_disabled(doc, name) {
        return false;
    }
    let format = logical_format(doc);
    if is_all(&format) {
        return true;
    }
    !find_vars(&format, name).is_empty()
}

/// Byte ranges of `$name` and `${name}`. `$c` does not match `$cmd_duration`.
pub fn find_vars(format: &str, name: &str) -> Vec<(usize, usize)> {
    let bytes = format.as_bytes();
    let needle = name.as_bytes();
    let mut hits = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'$' {
            i += 1;
            continue;
        }
        let start = i;
        i += 1;
        if i < bytes.len() && bytes[i] == b'{' {
            i += 1;
            let name_start = i;
            while i < bytes.len() && bytes[i] != b'}' {
                i += 1;
            }
            let name_end = i;
            if i < bytes.len() {
                i += 1;
            }
            if &bytes[name_start..name_end] == needle {
                hits.push((start, i));
            }
            continue;
        }
        let name_start = i;
        while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
            i += 1;
        }
        if &bytes[name_start..i] == needle {
            hits.push((start, i));
        }
    }
    hits
}

pub fn remove_var(format: &str, name: &str) -> String {
    let hits = find_vars(format, name);
    if hits.is_empty() {
        return format.to_string();
    }
    let mut out = String::new();
    let mut last = 0;
    for (start, end) in hits {
        out.push_str(&format[last..start]);
        last = end;
    }
    out.push_str(&format[last..]);
    out
}

/// Insert `$name` before `$line_break` or `$character` when those exist.
pub fn append_var(format: &str, name: &str) -> String {
    if !find_vars(format, name).is_empty() {
        return format.to_string();
    }
    let token = format!("${name}");
    if name != "line_break" {
        if let Some((at, _)) = find_vars(format, "line_break").into_iter().next() {
            let mut text = format.to_string();
            text.insert_str(at, &token);
            return text;
        }
    }
    if let Some((at, _)) = find_vars(format, "character").into_iter().next() {
        let mut text = format.to_string();
        text.insert_str(at, &token);
        return text;
    }
    format!("{format}{token}")
}

pub fn field_value(doc: &DocumentMut, module: Option<&str>, field: &str) -> String {
    let item = match module {
        Some(module) => doc
            .get(module)
            .and_then(Item::as_table)
            .and_then(|table| table.get(field)),
        None => doc.get(field),
    };
    let Some(item) = item else {
        return String::new();
    };
    match field {
        "disabled" | "add_newline" => item.as_bool().map(|v| v.to_string()).unwrap_or_default(),
        "scan_timeout" | "command_timeout" => item
            .as_integer()
            .map(|v| v.to_string())
            .unwrap_or_else(|| item.as_str().unwrap_or("").to_string()),
        _ => item.as_str().unwrap_or("").to_string(),
    }
}

pub fn set_field(
    doc: &mut DocumentMut,
    module: Option<&str>,
    field: &str,
    raw: &str,
) -> Result<(), String> {
    if raw.is_empty() {
        remove_field(doc, module, field);
        return Ok(());
    }
    let item = match field {
        "disabled" | "add_newline" => {
            let value = parse_bool(raw)?;
            toml_edit::value(value)
        }
        "scan_timeout" | "command_timeout" => {
            let value = raw
                .trim()
                .parse::<i64>()
                .map_err(|_| format!("{field} needs a number"))?;
            toml_edit::value(value)
        }
        _ => toml_edit::value(raw),
    };
    match module {
        Some(module) => {
            module_table(doc, module).insert(field, item);
        }
        None => {
            doc[field] = item;
        }
    }
    Ok(())
}

pub fn os_symbols(doc: &DocumentMut) -> Vec<(String, String)> {
    let Some(table) = doc
        .get("os")
        .and_then(Item::as_table)
        .and_then(|table| table.get("symbols"))
        .and_then(Item::as_table)
    else {
        return Vec::new();
    };
    let mut names: Vec<(String, String)> = table
        .iter()
        .filter_map(|(key, value)| {
            value
                .as_str()
                .map(|text| (key.to_string(), text.to_string()))
        })
        .collect();
    names.sort_by(|a, b| a.0.cmp(&b.0));
    names
}

pub fn set_os_symbol(doc: &mut DocumentMut, name: &str, value: &str) {
    let symbols = os_symbols_mut(doc);
    if value.is_empty() {
        symbols.remove(name);
    } else {
        symbols.insert(name, toml_edit::value(value));
    }
}

pub fn overlay_symbols(dest: &mut DocumentMut, src: &DocumentMut) {
    let keys: Vec<String> = src
        .iter()
        .filter_map(|(key, item)| item.as_table().map(|_| key.to_string()))
        .collect();
    for key in keys {
        if key == "palettes" || key == "profiles" {
            continue;
        }
        let Some(src_table) = src.get(&key).and_then(Item::as_table) else {
            continue;
        };
        if key == "os" {
            if let Some(symbols) = src_table.get("symbols").and_then(Item::as_table) {
                let pairs: Vec<(String, Item)> = symbols
                    .iter()
                    .map(|(name, value)| (name.to_string(), value.clone()))
                    .collect();
                let dest_symbols = os_symbols_mut(dest);
                for (name, value) in pairs {
                    dest_symbols.insert(&name, value);
                }
            }
            continue;
        }
        let fields: Vec<(String, Item)> = src_table
            .iter()
            .filter(|(field, value)| is_symbol_field(field) && !value.is_table())
            .map(|(field, value)| (field.to_string(), value.clone()))
            .collect();
        if fields.is_empty() {
            continue;
        }
        let dest_table = module_table(dest, &key);
        for (field, value) in fields {
            dest_table.insert(&field, value);
        }
    }
}

pub fn clear_symbols(doc: &mut DocumentMut) {
    let keys: Vec<String> = doc
        .iter()
        .filter_map(|(key, item)| item.as_table().map(|_| key.to_string()))
        .collect();
    for key in keys {
        let Some(table) = doc.get(&key).and_then(Item::as_table) else {
            continue;
        };
        let fields: Vec<String> = table
            .iter()
            .filter(|(field, value)| is_symbol_field(field) && !value.is_table())
            .map(|(field, _)| field.to_string())
            .collect();
        let has_os = key == "os" && table.get("symbols").and_then(Item::as_table).is_some();
        let os_names: Vec<String> = if has_os {
            table
                .get("symbols")
                .and_then(Item::as_table)
                .map(|symbols| symbols.iter().map(|(name, _)| name.to_string()).collect())
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        if !fields.is_empty() {
            let table = module_table(doc, &key);
            for field in fields {
                table.insert(&field, toml_edit::value(""));
            }
        }
        if has_os {
            let symbols = os_symbols_mut(doc);
            for name in os_names {
                symbols.insert(&name, toml_edit::value(""));
            }
        }
    }
}

fn is_symbol_field(field: &str) -> bool {
    field == "symbol" || field == "read_only" || field.ends_with("_symbol")
}

fn parse_bool(raw: &str) -> Result<bool, String> {
    match raw.trim() {
        "true" | "yes" | "on" => Ok(true),
        "false" | "no" | "off" => Ok(false),
        _ => Err("use true or false".to_string()),
    }
}

fn remove_field(doc: &mut DocumentMut, module: Option<&str>, field: &str) {
    match module {
        Some(module) => {
            if let Some(table) = doc.get_mut(module).and_then(Item::as_table_mut) {
                table.remove(field);
            }
        }
        None => {
            doc.remove(field);
        }
    }
}

fn module_table<'a>(doc: &'a mut DocumentMut, module: &str) -> &'a mut Table {
    let item = doc.entry(module).or_insert_with(toml_edit::table);
    if !item.is_table() {
        *item = toml_edit::table();
    }
    item.as_table_mut().expect("module table")
}

fn os_symbols_mut(doc: &mut DocumentMut) -> &mut Table {
    let os = module_table(doc, "os");
    let item = os.entry("symbols").or_insert_with(toml_edit::table);
    if !item.is_table() {
        *item = toml_edit::table();
    }
    item.as_table_mut().expect("os.symbols")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variable_boundaries_do_not_eat_longer_names() {
        let format = "$directory$c$cmd_duration$git_status$git_state$character";
        let removed = remove_var(format, "c");
        assert_eq!(
            removed,
            "$directory$cmd_duration$git_status$git_state$character"
        );
        assert!(find_vars(&removed, "git_state").len() == 1);
        assert!(find_vars(&remove_var(&removed, "git_status"), "git_state").len() == 1);
    }

    #[test]
    fn editor_save_keeps_comments_and_untouched_keys() {
        let original = "\
# keep me
\"$schema\" = \"https://starship.rs/config-schema.json\"

[rust]
symbol = \"R \"

[custom]
other = \"stay\"
";
        let mut doc = ConfigDoc::parse(original);
        set_field(&mut doc.doc, Some("rust"), "symbol", "X ").unwrap();
        set_field(&mut doc.doc, Some("rust"), "disabled", "true").unwrap();
        let saved = doc.to_toml();
        assert!(saved.contains("# keep me"), "{saved}");
        assert!(saved.contains("other = \"stay\""), "{saved}");
        assert!(saved.contains("symbol = \"X \""), "{saved}");
        assert!(saved.contains("disabled = true"), "{saved}");
        assert!(saved.contains("$schema"), "{saved}");
    }

    #[test]
    fn clearing_a_field_removes_the_key() {
        let mut doc = ConfigDoc::parse("[rust]\nsymbol = \"R \"\nstyle = \"red\"\n");
        set_field(&mut doc.doc, Some("rust"), "symbol", "").unwrap();
        let saved = doc.to_toml();
        assert!(!saved.contains("symbol"), "{saved}");
        assert!(saved.contains("style = \"red\""), "{saved}");
    }
}
