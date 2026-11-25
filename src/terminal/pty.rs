use std::io::{self, Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;

use eframe::egui;
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};

use crate::platform::shell::default_shell;

/// Bounded scrollback lines per terminal tab to prevent unbounded RAM growth
/// when AI coding agents stream large outputs.
pub const MAX_SCROLLBACK_LINES: usize = 4_000;

pub struct TerminalSession {
    pub title: String,
    pub parser: Arc<Mutex<vt100::Parser>>,
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    pub rows: u16,
    pub cols: u16,
    pub scroll_offset: usize,
    pub alive: Arc<std::sync::atomic::AtomicBool>,
}

impl TerminalSession {
    pub fn spawn(
        title: String,
        cwd: &Path,
        rows: u16,
        cols: u16,
        egui_ctx: egui::Context,
    ) -> Result<Self, String> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("openpty error: {e}"))?;

        let shell = default_shell();
        let mut cmd = CommandBuilder::new(&shell.program);
        for arg in &shell.args {
            cmd.arg(arg);
        }
        cmd.cwd(cwd);
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");

        let _child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| format!("spawn_command error: {e}"))?;

        // Drop slave handle in parent once child is spawned so EOF propagates cleanly.
        drop(pair.slave);

        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| format!("try_clone_reader error: {e}"))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|e| format!("take_writer error: {e}"))?;

        let parser = Arc::new(Mutex::new(vt100::Parser::new(
            rows,
            cols,
            MAX_SCROLLBACK_LINES,
        )));
        let alive = Arc::new(std::sync::atomic::AtomicBool::new(true));

        let parser_clone = Arc::clone(&parser);
        let alive_clone = Arc::clone(&alive);

        // Background PTY reader thread using a fixed stack buffer (zero heap allocs per read).
        thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if let Ok(mut p) = parser_clone.lock() {
                            p.process(&buf[..n]);
                        }
                        egui_ctx.request_repaint();
                    }
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(_) => break,
                }
            }
            alive_clone.store(false, std::sync::atomic::Ordering::Relaxed);
            egui_ctx.request_repaint();
        });

        Ok(Self {
            title,
            parser,
            master: pair.master,
            writer,
            rows,
            cols,
            scroll_offset: 0,
            alive,
        })
    }

    pub fn write_bytes(&mut self, data: &[u8]) {
        if !data.is_empty() {
            let _ = self.writer.write_all(data);
            let _ = self.writer.flush();
            self.scroll_offset = 0;
        }
    }

    pub fn send_command(&mut self, cmd: &str) {
        let mut payload = cmd.as_bytes().to_vec();
        payload.push(b'\r');
        self.write_bytes(&payload);
    }

    pub fn resize(&mut self, rows: u16, cols: u16) {
        let rows = rows.max(4);
        let cols = cols.max(16);
        if rows == self.rows && cols == self.cols {
            return;
        }
        self.rows = rows;
        self.cols = cols;
        let _ = self.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        });
        if let Ok(mut p) = self.parser.lock() {
            p.set_size(rows, cols);
        }
    }

    pub fn is_alive(&self) -> bool {
        self.alive.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn visible_text(&self) -> String {
        if let Ok(p) = self.parser.lock() {
            p.screen().contents()
        } else {
            String::new()
        }
    }
}
