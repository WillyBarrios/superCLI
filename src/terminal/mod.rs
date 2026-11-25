pub mod pty;
pub mod view;

use std::path::Path;

use eframe::egui;

use self::pty::TerminalSession;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalDock {
    Bottom,
    Right,
}

/// Manages multiple integrated terminal tabs optimized for AI coding agent CLIs.
pub struct TerminalManager {
    pub sessions: Vec<TerminalSession>,
    pub active_tab: usize,
    pub dock: TerminalDock,
    pub visible: bool,
    pub focused: bool,
    pub font_size: f32,
    next_id: usize,
}

impl TerminalManager {
    pub fn new() -> Self {
        Self {
            sessions: Vec::new(),
            active_tab: 0,
            dock: TerminalDock::Bottom,
            visible: true,
            focused: false,
            font_size: 13.5,
            next_id: 1,
        }
    }

    pub fn ensure_session(&mut self, cwd: &Path, ctx: &egui::Context) {
        if self.sessions.is_empty() {
            self.spawn_shell(cwd, None, None, ctx);
        }
    }

    pub fn spawn_shell(
        &mut self,
        cwd: &Path,
        title: Option<String>,
        initial_command: Option<&str>,
        ctx: &egui::Context,
    ) {
        let id = self.next_id;
        self.next_id += 1;
        let tab_title = title.unwrap_or_else(|| format!("Terminal {id}"));
        match TerminalSession::spawn(tab_title, cwd, 24, 80, ctx.clone()) {
            Ok(mut session) => {
                if let Some(cmd) = initial_command {
                    session.send_command(cmd);
                }
                self.sessions.push(session);
                self.active_tab = self.sessions.len() - 1;
                self.visible = true;
                self.focused = true;
            }
            Err(err) => {
                eprintln!("Failed to spawn terminal: {err}");
            }
        }
    }

    pub fn close_tab(&mut self, index: usize) {
        if index < self.sessions.len() {
            self.sessions.remove(index);
            if self.active_tab >= self.sessions.len() && !self.sessions.is_empty() {
                self.active_tab = self.sessions.len() - 1;
            }
        }
    }

    pub fn active_session_mut(&mut self) -> Option<&mut TerminalSession> {
        self.sessions.get_mut(self.active_tab)
    }
}
