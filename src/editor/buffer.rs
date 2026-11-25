use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::SystemTime;

use ropey::Rope;

const MAX_UNDO_HISTORY: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    Lf,
    CrLf,
}

impl LineEnding {
    pub fn label(self) -> &'static str {
        match self {
            LineEnding::Lf => "LF",
            LineEnding::CrLf => "CRLF",
        }
    }
}

/// Code document backed by `ropey::Rope` for O(log n) edits and minimal RAM usage.
pub struct DocumentBuffer {
    pub path: Option<PathBuf>,
    pub title: String,
    pub rope: Rope,
    /// Cached text view synchronized on edit for egui's TextEdit widget.
    pub text_cache: String,
    pub dirty: bool,
    pub line_ending: LineEnding,
    pub last_modified: Option<SystemTime>,
    undo_stack: Vec<String>,
    redo_stack: Vec<String>,
}

impl DocumentBuffer {
    pub fn new_untitled() -> Self {
        Self {
            path: None,
            title: "sin_titulo.txt".to_string(),
            rope: Rope::new(),
            text_cache: String::new(),
            dirty: false,
            line_ending: if cfg!(windows) {
                LineEnding::CrLf
            } else {
                LineEnding::Lf
            },
            last_modified: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    pub fn from_file(path: PathBuf) -> io::Result<Self> {
        let raw = fs::read_to_string(&path)?;
        let line_ending = if raw.contains("\r\n") {
            LineEnding::CrLf
        } else {
            LineEnding::Lf
        };
        let normalized = if line_ending == LineEnding::CrLf {
            raw.replace("\r\n", "\n")
        } else {
            raw
        };
        let rope = Rope::from_str(&normalized);
        let last_modified = fs::metadata(&path).ok().and_then(|m| m.modified().ok());
        let title = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| path.display().to_string());

        Ok(Self {
            path: Some(path),
            title,
            rope,
            text_cache: normalized,
            dirty: false,
            line_ending,
            last_modified,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        })
    }

    pub fn extension(&self) -> &str {
        self.path
            .as_deref()
            .and_then(|p| p.extension())
            .and_then(|e| e.to_str())
            .unwrap_or("")
    }

    pub fn line_count(&self) -> usize {
        self.rope.len_lines().max(1)
    }

    /// Commits a change from `text_cache` back into the `Rope` and pushes bounded undo history.
    pub fn commit_edit(&mut self, previous: String) {
        if self.undo_stack.len() >= MAX_UNDO_HISTORY {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(previous);
        self.redo_stack.clear();
        self.rope = Rope::from_str(&self.text_cache);
        self.dirty = true;
    }

    pub fn undo(&mut self) -> bool {
        if let Some(prev) = self.undo_stack.pop() {
            let current = std::mem::replace(&mut self.text_cache, prev);
            self.redo_stack.push(current);
            self.rope = Rope::from_str(&self.text_cache);
            self.dirty = true;
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self) -> bool {
        if let Some(next) = self.redo_stack.pop() {
            let current = std::mem::replace(&mut self.text_cache, next);
            self.undo_stack.push(current);
            self.rope = Rope::from_str(&self.text_cache);
            self.dirty = true;
            true
        } else {
            false
        }
    }

    pub fn save(&mut self) -> io::Result<()> {
        let Some(path) = self.path.as_ref() else {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "El archivo no tiene ruta asignada",
            ));
        };
        self.rope = Rope::from_str(&self.text_cache);
        let output = match self.line_ending {
            LineEnding::Lf => self.rope.to_string(),
            LineEnding::CrLf => self.rope.to_string().replace('\n', "\r\n"),
        };
        fs::write(path, output)?;
        self.dirty = false;
        self.last_modified = fs::metadata(path).ok().and_then(|m| m.modified().ok());
        Ok(())
    }

    /// Automatically reloads the file if an AI CLI agent modified it on disk and buffer is clean.
    pub fn reload_if_changed_on_disk(&mut self) -> bool {
        if self.dirty {
            return false;
        }
        let Some(path) = self.path.as_ref() else {
            return false;
        };
        let disk_mtime = fs::metadata(path).ok().and_then(|m| m.modified().ok());
        if disk_mtime.is_some() && disk_mtime != self.last_modified {
            if let Ok(raw) = fs::read_to_string(path) {
                let normalized = raw.replace("\r\n", "\n");
                self.rope = Rope::from_str(&normalized);
                self.text_cache = normalized;
                self.last_modified = disk_mtime;
                self.undo_stack.clear();
                self.redo_stack.clear();
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_and_redo_work_with_rope() {
        let mut buf = DocumentBuffer::new_untitled();
        let prev = buf.text_cache.clone();
        buf.text_cache = "fn main() {}".to_string();
        buf.commit_edit(prev);
        assert!(buf.dirty);
        assert_eq!(buf.rope.to_string(), "fn main() {}");

        assert!(buf.undo());
        assert_eq!(buf.rope.to_string(), "");

        assert!(buf.redo());
        assert_eq!(buf.rope.to_string(), "fn main() {}");
    }
}
