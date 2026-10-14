use crate::config;
use std::path::PathBuf;
use toml_edit::DocumentMut;

pub struct Workspace {
    pub path: PathBuf,
    pub baseline: String,
    pub doc: DocumentMut,
    pub raw: String,
    pub error: Option<String>,
    pub load_error: Option<String>,
    pub backup: Option<PathBuf>,
    undo: Vec<String>,
    redo: Vec<String>,
    existed: bool,
}
impl Workspace {
    pub fn open(path: PathBuf) -> Self {
        let existed = path.exists();
        let (baseline, load_error) = match config::read_config(&path) {
            Ok(s) => (s, None),
            Err(e) => (String::new(), Some(format!("{e:#}"))),
        };
        let parsed = baseline.parse::<DocumentMut>();
        let error = parsed.as_ref().err().map(ToString::to_string);
        Self {
            path,
            raw: baseline.clone(),
            baseline,
            doc: parsed.unwrap_or_default(),
            error,
            load_error,
            backup: None,
            undo: vec![],
            redo: vec![],
            existed,
        }
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn dirty(&self) -> bool {
        self.raw != self.baseline
    }
    pub fn adopt(&mut self, text: String) -> Result<(), String> {
        let doc = text.parse::<DocumentMut>().map_err(|e| e.to_string())?;
        if text != self.doc.to_string() {
            self.undo.push(self.doc.to_string());
            self.redo.clear();
        }
        self.doc = doc;
        self.raw = text;
        self.error = None;
        Ok(())
    }
    pub fn apply_raw(&mut self) -> Result<(), String> {
        match self.adopt(self.raw.clone()) {
            Ok(()) => Ok(()),
            Err(e) => {
                self.error = Some(e.clone());
                Err(e)
            }
        }
    }
    pub fn edit(&mut self, f: impl FnOnce(&mut DocumentMut)) {
        if self.error.is_some() || self.load_error.is_some() {
            return;
        }
        if self.raw != self.doc.to_string() && self.apply_raw().is_err() {
            return;
        }
        let mut doc = self.doc.clone();
        f(&mut doc);
        let _ = self.adopt(doc.to_string());
    }
    pub fn undo(&mut self, redo: bool) {
        let source = if redo { &mut self.redo } else { &mut self.undo };
        if let Some(text) = source.pop() {
            let current = self.doc.to_string();
            if redo {
                self.undo.push(current);
            } else {
                self.redo.push(current);
            }
            self.doc = text.parse().expect("valid history");
            self.raw = text;
            self.error = None;
        }
    }
    pub fn save(&mut self, destination: Option<PathBuf>) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.load_error.is_none(),
            "Unreadable source: reopen after fixing access before saving"
        );
        self.apply_raw().map_err(anyhow::Error::msg)?;
        config::validate(&self.doc).map_err(anyhow::Error::msg)?;
        let target = destination.as_ref().unwrap_or(&self.path);
        if target == &self.path {
            anyhow::ensure!(
                target.exists() == self.existed && config::read_config(target)? == self.baseline,
                "Configuration changed outside Aster. Reopen or Save as to another file."
            );
        } else {
            anyhow::ensure!(
                !target.exists(),
                "Save as target already exists; choose a new path"
            );
        }
        self.backup = config::write_config(target, &self.raw, true)?;
        self.path = target.clone();
        self.baseline = self.raw.clone();
        self.existed = true;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn history_and_invalid_text() {
        let dir = tempfile::tempdir().unwrap();
        let mut w = Workspace::open(dir.path().join("s.toml"));
        w.edit(|d| d["format"] = toml_edit::value("$directory"));
        let valid = w.raw.clone();
        w.raw = "bad = [".into();
        assert!(w.apply_raw().is_err());
        assert_eq!(w.doc.to_string(), valid);
        assert_eq!(w.raw, "bad = [");
        w.undo(false);
        assert!(w.raw.is_empty());
        w.undo(true);
        assert_eq!(w.raw, valid);
    }
    #[test]
    fn external_change_and_backups() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.toml");
        std::fs::write(&path, "# stay\nx = 1\n").unwrap();
        let mut w = Workspace::open(path.clone());
        w.edit(|d| d["x"] = toml_edit::value(2));
        std::fs::write(&path, "x = 3\n").unwrap();
        assert!(w.save(None).is_err());
        assert!(w.dirty());
        std::fs::write(&path, &w.baseline).unwrap();
        w.save(None).unwrap();
        assert!(w.backup.as_ref().unwrap().exists());
        assert!(w.raw.contains("# stay"));
    }
    #[test]
    fn malformed_source_cannot_save() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.toml");
        std::fs::write(&path, "invalid = [").unwrap();
        let mut w = Workspace::open(path.clone());
        assert!(w.save(None).is_err());
        assert_eq!(std::fs::read_to_string(path).unwrap(), "invalid = [");
    }
    #[test]
    fn failed_save_retains_draft() {
        let dir = tempfile::tempdir().unwrap();
        let blocker = dir.path().join("file");
        std::fs::write(&blocker, "block").unwrap();
        let mut w = Workspace::open(blocker.join("s.toml"));
        assert!(w.load_error.is_some());
        w.raw = "format = '$character'".into();
        assert!(w.save(None).is_err());
        assert!(w.dirty());
        assert_eq!(std::fs::read_to_string(blocker).unwrap(), "block");
    }
    #[test]
    fn symlink_permissions_and_unique_backups() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("real.toml");
        let link = dir.path().join("link.toml");
        std::fs::write(&target, "x = 1\n").unwrap();
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o640)).unwrap();
        symlink(&target, &link).unwrap();
        let mut w = Workspace::open(link.clone());
        w.edit(|d| d["x"] = toml_edit::value(2));
        w.save(None).unwrap();
        let first = w.backup.clone().unwrap();
        w.edit(|d| d["x"] = toml_edit::value(3));
        w.save(None).unwrap();
        assert_ne!(first, w.backup.clone().unwrap());
        assert!(link.is_symlink());
        assert_eq!(
            std::fs::metadata(&target).unwrap().permissions().mode() & 0o777,
            0o640
        );
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o440)).unwrap();
        w.edit(|d| d["x"] = toml_edit::value(4));
        assert!(w.save(None).is_err());
        assert!(w.dirty());
    }
    #[test]
    fn raw_edits_survive_other_controls() {
        let dir = tempfile::tempdir().unwrap();
        let mut w = Workspace::open(dir.path().join("s"));
        w.raw = "# retained\nformat = '$directory'\n".into();
        w.edit(|d| d["right_format"] = toml_edit::value("$jobs"));
        assert!(w.raw.contains("# retained"));
        assert_eq!(w.doc["format"].as_str(), Some("$directory"));
        w.undo(false);
        assert!(w.raw.contains("$directory"));
    }
    #[test]
    fn save_as_collision_and_invalid_known_type() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("exists");
        std::fs::write(&path, "x=1").unwrap();
        let mut w = Workspace::open(dir.path().join("s"));
        w.raw = "format = 7".into();
        assert!(w.save(None).is_err());
        w.raw = "format = '$character'".into();
        assert!(w.save(Some(path.clone())).is_err());
        assert_eq!(std::fs::read_to_string(path).unwrap(), "x=1");
    }
    #[test]
    fn unknown_metadata_is_preserved_without_assuming_its_schema() {
        let dir = tempfile::tempdir().unwrap();
        let mut w = Workspace::open(dir.path().join("s.toml"));
        let text = "symbol=7\n[future_metadata]\nformat=42 # keep this\ndisabled='metadata'\n";
        w.adopt(text.into()).unwrap();
        w.save(None).unwrap();
        assert_eq!(std::fs::read_to_string(&w.path).unwrap(), text);
    }
}
