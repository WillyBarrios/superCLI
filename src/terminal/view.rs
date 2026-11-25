use std::path::Path;

use eframe::egui::{self, Color32, FontId, Key, Pos2, Rect, Sense, Stroke, Vec2};

use crate::platform::shell::AGENT_PRESETS;

use super::{TerminalDock, TerminalManager};

const TERM_BG: Color32 = Color32::from_rgb(18, 20, 26);
const TERM_FG: Color32 = Color32::from_rgb(212, 218, 232);
const CURSOR_BG: Color32 = Color32::from_rgb(122, 162, 247);

pub fn render_terminal_panel(
    ui: &mut egui::Ui,
    term: &mut TerminalManager,
    project_root: &Path,
    ctx: &egui::Context,
) {
    // Top terminal toolbar: wrapped layout so the Right-docked panel can always be resized freely
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);

        let mut close_idx = None;
        for (i, session) in term.sessions.iter().enumerate() {
            let selected = i == term.active_tab;
            let status_dot = if session.is_alive() { "*" } else { "o" };
            let label = format!("{status_dot} {}", session.title);
            if ui.selectable_label(selected, label).clicked() {
                term.active_tab = i;
                term.focused = true;
            }
            if term.sessions.len() > 1 && ui.small_button("x").clicked() {
                close_idx = Some(i);
            }
        }
        if let Some(idx) = close_idx {
            term.close_tab(idx);
        }

        if ui.small_button("+ Shell").on_hover_text("Nueva terminal").clicked() {
            term.spawn_shell(project_root, None, None, ctx);
        }

        ui.menu_button("Agentes v", |ui| {
            for preset in AGENT_PRESETS {
                if ui
                    .button(format!("{} ({})", preset.name, preset.command))
                    .clicked()
                {
                    term.spawn_shell(
                        project_root,
                        Some(preset.name.to_string()),
                        Some(preset.command),
                        ctx,
                    );
                    ui.close_menu();
                }
            }
        });

        let dock_label = match term.dock {
            TerminalDock::Bottom => "Derecha",
            TerminalDock::Right => "Abajo",
        };
        if ui
            .small_button(dock_label)
            .on_hover_text("Cambiar posición de la terminal")
            .clicked()
        {
            term.dock = match term.dock {
                TerminalDock::Bottom => TerminalDock::Right,
                TerminalDock::Right => TerminalDock::Bottom,
            };
        }

        if ui.small_button("x").on_hover_text("Ocultar terminal").clicked() {
            term.visible = false;
            term.focused = false;
        }
    });

    ui.separator();

    term.ensure_session(project_root, ctx);
    let font_size = term.font_size;
    let mut is_focused = term.focused;

    let Some(session) = term.active_session_mut() else {
        return;
    };

    let available = ui.available_size();
    let (rect, response) = ui.allocate_exact_size(available, Sense::click());

    if response.clicked() {
        is_focused = true;
    }

    let font_id = FontId::monospace(font_size);
    let cell_width = ui.fonts(|f| f.glyph_width(&font_id, 'M')).max(6.0);
    let cell_height = (font_size * 1.32).ceil().max(12.0);

    let cols = ((rect.width() - 8.0) / cell_width).floor().max(16.0) as u16;
    let rows = ((rect.height() - 6.0) / cell_height).floor().max(4.0) as u16;
    session.resize(rows, cols);

    // Handle mouse wheel scrollback when hovering terminal
    if response.hovered() {
        let scroll_delta = ui.input(|i| i.raw_scroll_delta.y);
        if scroll_delta.abs() > 0.1 {
            let lines = (scroll_delta / 24.0).round() as isize;
            if lines > 0 {
                session.scroll_offset = (session.scroll_offset + lines as usize).min(4_000);
            } else {
                session.scroll_offset = session.scroll_offset.saturating_sub((-lines) as usize);
            }
        }
    }

    // Handle keyboard input when terminal is focused
    if is_focused {
        let events = ui.input(|i| i.events.clone());
        for event in events {
            match event {
                egui::Event::Text(text) => {
                    session.write_bytes(text.as_bytes());
                }
                egui::Event::Paste(text) => {
                    session.write_bytes(text.as_bytes());
                }
                egui::Event::Copy => {
                    let text = session.visible_text();
                    ctx.copy_text(text);
                }
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => {
                    if modifiers.ctrl && modifiers.shift && key == Key::C {
                        let text = session.visible_text();
                        ctx.copy_text(text);
                    } else if let Some(seq) = encode_key(key, modifiers) {
                        session.write_bytes(&seq);
                    }
                }
                _ => {}
            }
        }
    }

    // Paint terminal grid with batched horizontal runs for low CPU/GPU overhead
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, TERM_BG);

    if is_focused {
        painter.rect_stroke(
            rect.shrink(0.5),
            0.0,
            Stroke::new(1.0_f32, Color32::from_rgb(80, 110, 180)),
        );
    }

    let origin = Pos2::new(rect.left() + 4.0, rect.top() + 3.0);

    if let Ok(mut parser) = session.parser.lock() {
        parser.set_scrollback(session.scroll_offset);
        let screen = parser.screen();
        let (screen_rows, screen_cols) = screen.size();
        let (cursor_row, cursor_col) = screen.cursor_position();
        let hide_cursor = screen.hide_cursor() || session.scroll_offset > 0;

        let mut run_text = String::with_capacity(screen_cols as usize);

        for row in 0..screen_rows {
            let y = origin.y + row as f32 * cell_height;
            let mut col = 0u16;
            while col < screen_cols {
                let Some(cell) = screen.cell(row, col) else {
                    col += 1;
                    continue;
                };
                if cell.is_wide_continuation() {
                    col += 1;
                    continue;
                }

                let (mut fg, mut bg) = (
                    vt_color_to_egui(cell.fgcolor(), true),
                    vt_color_to_egui(cell.bgcolor(), false),
                );
                if cell.inverse() {
                    std::mem::swap(&mut fg, &mut bg);
                }

                let start_col = col;
                run_text.clear();

                // Merge consecutive cells with identical fg & bg into a single draw call
                while col < screen_cols {
                    let Some(c) = screen.cell(row, col) else {
                        break;
                    };
                    if c.is_wide_continuation() {
                        col += 1;
                        continue;
                    }
                    let (mut c_fg, mut c_bg) = (
                        vt_color_to_egui(c.fgcolor(), true),
                        vt_color_to_egui(c.bgcolor(), false),
                    );
                    if c.inverse() {
                        std::mem::swap(&mut c_fg, &mut c_bg);
                    }
                    if c_fg != fg || c_bg != bg {
                        break;
                    }
                    let contents = c.contents();
                    if contents.is_empty() {
                        run_text.push(' ');
                    } else {
                        run_text.push_str(&contents);
                    }
                    col += 1;
                }

                let run_x = origin.x + start_col as f32 * cell_width;
                let run_w = (col - start_col) as f32 * cell_width;

                if bg != TERM_BG {
                    painter.rect_filled(
                        Rect::from_min_size(Pos2::new(run_x, y), Vec2::new(run_w, cell_height)),
                        0.0,
                        bg,
                    );
                }

                if run_text.bytes().any(|b| b != b' ') {
                    painter.text(
                        Pos2::new(run_x, y),
                        egui::Align2::LEFT_TOP,
                        &run_text,
                        font_id.clone(),
                        fg,
                    );
                }
            }
        }

        // Draw terminal cursor
        if !hide_cursor && cursor_row < screen_rows && cursor_col < screen_cols {
            let cx = origin.x + cursor_col as f32 * cell_width;
            let cy = origin.y + cursor_row as f32 * cell_height;
            let cursor_rect = Rect::from_min_size(
                Pos2::new(cx, cy),
                Vec2::new(if is_focused { cell_width } else { 2.0 }, cell_height),
            );
            if is_focused {
                painter.rect_filled(cursor_rect, 0.0, CURSOR_BG.linear_multiply(0.75));
            } else {
                painter.rect_stroke(cursor_rect, 0.0, Stroke::new(1.0_f32, CURSOR_BG));
            }
        }
    }

    term.focused = is_focused;
}

fn encode_key(key: Key, modifiers: egui::Modifiers) -> Option<Vec<u8>> {
    if modifiers.ctrl && !modifiers.alt && !modifiers.shift {
        let ctrl_byte = match key {
            Key::A => Some(0x01),
            Key::B => Some(0x02),
            Key::C => Some(0x03),
            Key::D => Some(0x04),
            Key::E => Some(0x05),
            Key::F => Some(0x06),
            Key::G => Some(0x07),
            Key::H => Some(0x08),
            Key::K => Some(0x0B),
            Key::L => Some(0x0C),
            Key::N => Some(0x0E),
            Key::P => Some(0x10),
            Key::R => Some(0x12),
            Key::U => Some(0x15),
            Key::W => Some(0x17),
            Key::Z => Some(0x1A),
            _ => None,
        };
        if let Some(b) = ctrl_byte {
            return Some(vec![b]);
        }
    }

    let bytes: &[u8] = match key {
        Key::Enter => b"\r",
        Key::Backspace => b"\x7f",
        Key::Tab => b"\t",
        Key::Escape => b"\x1b",
        Key::ArrowUp => b"\x1b[A",
        Key::ArrowDown => b"\x1b[B",
        Key::ArrowRight => b"\x1b[C",
        Key::ArrowLeft => b"\x1b[D",
        Key::Home => b"\x1b[H",
        Key::End => b"\x1b[F",
        Key::Delete => b"\x1b[3~",
        Key::PageUp => b"\x1b[5~",
        Key::PageDown => b"\x1b[6~",
        _ => return None,
    };
    Some(bytes.to_vec())
}

fn vt_color_to_egui(color: vt100::Color, is_fg: bool) -> Color32 {
    match color {
        vt100::Color::Default => {
            if is_fg {
                TERM_FG
            } else {
                TERM_BG
            }
        }
        vt100::Color::Rgb(r, g, b) => Color32::from_rgb(r, g, b),
        vt100::Color::Idx(idx) => ansi_256_color(idx),
    }
}

fn ansi_256_color(idx: u8) -> Color32 {
    const ANSI_16: [(u8, u8, u8); 16] = [
        (24, 26, 36),    // Black
        (247, 118, 142), // Red
        (158, 206, 106), // Green
        (224, 175, 104), // Yellow
        (122, 162, 247), // Blue
        (187, 154, 247), // Magenta
        (125, 207, 255), // Cyan
        (192, 202, 245), // White
        (86, 95, 137),   // Bright Black
        (255, 137, 157), // Bright Red
        (175, 224, 128), // Bright Green
        (240, 195, 120), // Bright Yellow
        (140, 180, 255), // Bright Blue
        (205, 175, 255), // Bright Magenta
        (150, 225, 255), // Bright Cyan
        (235, 240, 255), // Bright White
    ];
    if idx < 16 {
        let (r, g, b) = ANSI_16[idx as usize];
        return Color32::from_rgb(r, g, b);
    }
    if idx < 232 {
        let n = idx - 16;
        let r = (n / 36) * 51;
        let g = ((n % 36) / 6) * 51;
        let b = (n % 6) * 51;
        return Color32::from_rgb(r, g, b);
    }
    let gray = 8 + (idx - 232) * 10;
    Color32::from_rgb(gray, gray, gray)
}
