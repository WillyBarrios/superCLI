pub mod buffer;
pub mod syntax;
pub mod view;

use std::path::{Path, PathBuf};

use self::buffer::DocumentBuffer;

/// Manages open code tabs, search state, and active document cursor info.
pub struct EditorManager {
    pub tabs: Vec<DocumentBuffer>,
    pub active_tab: usize,
    pub font_size: f32,
    pub find_open: bool,
    pub find_query: String,
    pub status_message: Option<String>,
}

impl EditorManager {
    pub fn new() -> Self {
        Self {
            tabs: Vec::new(),
            active_tab: 0,
            font_size: 14.0,
            find_open: false,
            find_query: String::new(),
            status_message: None,
        }
    }

    pub fn open_file(&mut self, path: PathBuf) {
        if let Some(existing_idx) = self
            .tabs
            .iter()
            .position(|t| t.path.as_deref() == Some(path.as_path()))
        {
            self.active_tab = existing_idx;
            return;
        }

        match DocumentBuffer::from_file(path) {
            Ok(doc) => {
                self.tabs.push(doc);
                self.active_tab = self.tabs.len() - 1;
                self.status_message = None;
            }
            Err(err) => {
                self.status_message = Some(format!("No se pudo abrir el archivo: {err}"));
            }
        }
    }

    pub fn new_untitled(&mut self) {
        self.tabs.push(DocumentBuffer::new_untitled());
        self.active_tab = self.tabs.len() - 1;
    }

    pub fn close_tab(&mut self, idx: usize) {
        if idx < self.tabs.len() {
            self.tabs.remove(idx);
            if self.active_tab >= self.tabs.len() && !self.tabs.is_empty() {
                self.active_tab = self.tabs.len() - 1;
            }
        }
    }

    pub fn active_doc(&self) -> Option<&DocumentBuffer> {
        self.tabs.get(self.active_tab)
    }

    pub fn active_doc_mut(&mut self) -> Option<&mut DocumentBuffer> {
        self.tabs.get_mut(self.active_tab)
    }

    pub fn save_active(&mut self) {
        if let Some(doc) = self.tabs.get_mut(self.active_tab) {
            match doc.save() {
                Ok(()) => {
                    self.status_message = Some(format!("Guardado: {}", doc.title));
                }
                Err(err) => {
                    self.status_message = Some(format!("Error al guardar: {err}"));
                }
            }
        }
    }

    /// Reloads any clean open buffers whose backing files were modified on disk (e.g. by AI agents).
    pub fn reload_changed_paths(&mut self, changed_paths: &[PathBuf]) {
        for doc in &mut self.tabs {
            let matches_any = doc.path.as_deref().map_or(false, |p| {
                changed_paths
                    .iter()
                    .any(|c| c == p || p.starts_with(c) || c.parent() == p.parent())
            });
            if matches_any && doc.reload_if_changed_on_disk() {
                self.status_message = Some(format!("Recargado desde disco: {}", doc.title));
            }
        }
    }

    pub fn is_open(&self, path: &Path) -> bool {
        self.tabs.iter().any(|t| t.path.as_deref() == Some(path))
    }

    /// Updates any open tab's path and title after a file is renamed in the project explorer.
    pub fn notify_renamed(&mut self, old_path: &Path, new_path: &Path) {
        for doc in &mut self.tabs {
            if doc.path.as_deref() == Some(old_path) {
                doc.path = Some(new_path.to_path_buf());
                doc.title = new_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| new_path.display().to_string());
            }
        }
    }
}
