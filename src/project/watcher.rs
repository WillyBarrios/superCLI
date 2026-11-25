use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};

use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};

/// Lightweight filesystem watcher that watches only expanded directories (non-recursively)
/// to minimize RAM and avoid exhausting Linux `inotify` watch limits.
pub struct ProjectWatcher {
    watcher: Option<RecommendedWatcher>,
    rx: Receiver<PathBuf>,
    watched_dirs: HashSet<PathBuf>,
}

impl ProjectWatcher {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel::<PathBuf>();
        let watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    for path in event.paths {
                        let _ = tx.send(path);
                    }
                }
            },
            Config::default(),
        )
        .ok();

        Self {
            watcher,
            rx,
            watched_dirs: HashSet::new(),
        }
    }

    pub fn watch_dir(&mut self, dir: &Path) {
        if self.watched_dirs.contains(dir) {
            return;
        }
        if let Some(watcher) = self.watcher.as_mut() {
            if watcher.watch(dir, RecursiveMode::NonRecursive).is_ok() {
                self.watched_dirs.insert(dir.to_path_buf());
            }
        }
    }

    pub fn unwatch_dir(&mut self, dir: &Path) {
        if self.watched_dirs.remove(dir) {
            if let Some(watcher) = self.watcher.as_mut() {
                let _ = watcher.unwatch(dir);
            }
        }
    }

    pub fn clear(&mut self) {
        if let Some(watcher) = self.watcher.as_mut() {
            for dir in self.watched_dirs.drain() {
                let _ = watcher.unwatch(&dir);
            }
        } else {
            self.watched_dirs.clear();
        }
    }

    /// Drains pending filesystem change notifications non-blockingly and deduplicates paths.
    pub fn poll_changes(&self) -> Vec<PathBuf> {
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        while let Ok(path) = self.rx.try_recv() {
            if seen.insert(path.clone()) {
                out.push(path);
            }
        }
        out
    }
}
