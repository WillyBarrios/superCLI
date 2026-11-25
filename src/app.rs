use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, Key, RichText};

use crate::editor::view::render_editor_panel;
use crate::editor::EditorManager;
use crate::platform::memory::current_ram_mb;
use crate::platform::shell::default_shell;
use crate::project::{ProjectTree, TreeNode};
use crate::terminal::view::render_terminal_panel;
use crate::terminal::{TerminalDock, TerminalManager};

pub struct SuperCliApp {
    project: ProjectTree,
    editor: EditorManager,
    terminal: TerminalManager,
    left_panel_open: bool,
    quick_open_visible: bool,
    quick_open_query: String,
    quick_open_results: Vec<PathBuf>,
    new_item_dialog: Option<NewItemMode>,
    new_item_name: String,
    cached_ram_mb: f64,
    last_ram_sample: Instant,
    shell_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NewItemMode {
    File,
    Directory,
}

impl SuperCliApp {
    pub fn new(cc: &eframe::CreationContext<'_>, root_dir: PathBuf) -> Self {
        apply_dark_theme(&cc.egui_ctx);
        let shell = default_shell();
        Self {
            project: ProjectTree::new(root_dir),
            editor: EditorManager::new(),
            terminal: TerminalManager::new(),
            left_panel_open: true,
            quick_open_visible: false,
            quick_open_query: String::new(),
            quick_open_results: Vec::new(),
            new_item_dialog: None,
            new_item_name: String::new(),
            cached_ram_mb: current_ram_mb(),
            last_ram_sample: Instant::now(),
            shell_name: shell.display_name,
        }
    }

    fn handle_global_shortcuts(&mut self, ctx: &egui::Context) {
        let mut toggle_left = false;
        let mut toggle_term = false;
        let mut save_file = false;
        let mut open_quick = false;
        let mut open_find = false;
        let mut undo = false;
        let mut redo = false;

        ctx.input(|i| {
            if i.modifiers.ctrl && !i.modifiers.shift {
                if i.key_pressed(Key::B) {
                    toggle_left = true;
                }
                if i.key_pressed(Key::J) || i.key_pressed(Key::Backtick) {
                    toggle_term = true;
                }
                if i.key_pressed(Key::S) {
                    save_file = true;
                }
                if i.key_pressed(Key::P) {
                    open_quick = true;
                }
                if i.key_pressed(Key::F) && !self.terminal.focused {
                    open_find = true;
                }
                if i.key_pressed(Key::Z) && !self.terminal.focused {
                    undo = true;
                }
                if i.key_pressed(Key::Y) && !self.terminal.focused {
                    redo = true;
                }
            }
        });

        if toggle_left {
            self.left_panel_open = !self.left_panel_open;
        }
        if toggle_term {
            self.terminal.visible = !self.terminal.visible;
            self.terminal.focused = self.terminal.visible;
        }
        if save_file {
            self.editor.save_active();
        }
        if open_quick {
            self.quick_open_visible = !self.quick_open_visible;
            if self.quick_open_visible {
                self.quick_open_query.clear();
                self.quick_open_results = self.project.find_files("", 40);
                self.terminal.focused = false;
            }
        }
        if open_find {
            self.editor.find_open = !self.editor.find_open;
        }
        if undo {
            if let Some(doc) = self.editor.active_doc_mut() {
                doc.undo();
            }
        }
        if redo {
            if let Some(doc) = self.editor.active_doc_mut() {
                doc.redo();
            }
        }
    }

    fn poll_filesystem_and_metrics(&mut self) {
        let changed = self.project.watcher.poll_changes();
        if !changed.is_empty() {
            self.project.refresh();
            self.editor.reload_changed_paths(&changed);
        }

        if self.last_ram_sample.elapsed() >= Duration::from_secs(1) {
            self.cached_ram_mb = current_ram_mb();
            self.last_ram_sample = Instant::now();
        }
    }

    fn render_top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("top_bar")
            .exact_height(30.0)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.strong(RichText::new("superCLI").color(Color32::from_rgb(137, 180, 250)));
                    ui.separator();

                    if ui.small_button("Abrir carpeta").clicked() {
                        if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                            self.project.set_root(folder.clone());
                            self.terminal.spawn_shell(&folder, None, None, ctx);
                        }
                    }

                    if ui.small_button("Buscar (Ctrl+P)").clicked() {
                        self.quick_open_visible = true;
                        self.quick_open_query.clear();
                        self.quick_open_results = self.project.find_files("", 40);
                        self.terminal.focused = false;
                    }

                    ui.separator();

                    if ui
                        .selectable_label(self.left_panel_open, "Proyecto")
                        .clicked()
                    {
                        self.left_panel_open = !self.left_panel_open;
                    }
                    if ui
                        .selectable_label(self.terminal.visible, "Terminal")
                        .clicked()
                    {
                        self.terminal.visible = !self.terminal.visible;
                        self.terminal.focused = self.terminal.visible;
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(self.project.root.display().to_string())
                                .small()
                                .weak(),
                        );
                    });
                });
            });
    }

    fn render_status_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status_bar")
            .exact_height(24.0)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    let ram_color = if self.cached_ram_mb < 60.0 {
                        Color32::from_rgb(166, 227, 161)
                    } else {
                        Color32::from_rgb(249, 226, 175)
                    };
                    ui.label(
                        RichText::new(format!("RAM: {:.1} MB", self.cached_ram_mb))
                            .small()
                            .strong()
                            .color(ram_color),
                    );
                    ui.separator();
                    ui.label(RichText::new(format!("Shell: {}", self.shell_name)).small().weak());
                    ui.separator();

                    if let Some(doc) = self.editor.active_doc() {
                        let path_str = doc
                            .path
                            .as_deref()
                            .map(|p| p.display().to_string())
                            .unwrap_or_else(|| doc.title.clone());
                        ui.label(RichText::new(path_str).small());

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                RichText::new(format!(
                                    "{} líneas  |  UTF-8  |  {}",
                                    doc.line_count(),
                                    doc.line_ending.label()
                                ))
                                .small()
                                .weak(),
                            );
                        });
                    } else {
                        ui.label(RichText::new("Sin archivo activo").small().weak());
                    }
                });
            });
    }

    fn render_left_project_panel(&mut self, ctx: &egui::Context) {
        if !self.left_panel_open {
            return;
        }

        egui::SidePanel::left("project_explorer")
            .resizable(true)
            .default_width(240.0)
            .width_range(150.0..=450.0)
            .show(ctx, |ui| {
                // Prevent child widgets from pushing the SidePanel wider than the user's chosen width.
                let panel_w = ui.available_width();
                ui.set_max_width(panel_w);

                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("R").on_hover_text("Refrescar").clicked() {
                            self.project.refresh();
                        }
                        if ui
                            .small_button("+Dir")
                            .on_hover_text("Nueva carpeta")
                            .clicked()
                        {
                            self.new_item_dialog = Some(NewItemMode::Directory);
                            self.new_item_name.clear();
                            self.terminal.focused = false;
                        }
                        if ui
                            .small_button("+Arch")
                            .on_hover_text("Nuevo archivo")
                            .clicked()
                        {
                            self.new_item_dialog = Some(NewItemMode::File);
                            self.new_item_name.clear();
                            self.terminal.focused = false;
                        }
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(self.project.root_name.to_uppercase()).strong(),
                                )
                                .truncate(),
                            );
                        });
                    });
                });

                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .selectable_label(self.project.show_hidden, ".*")
                            .on_hover_text("Mostrar archivos ocultos")
                            .clicked()
                        {
                            self.project.show_hidden = !self.project.show_hidden;
                            self.project.refresh();
                        }
                        let filter_w = (ui.available_width() - 4.0).max(40.0);
                        let resp = ui.add(
                            egui::TextEdit::singleline(&mut self.project.filter_query)
                                .desired_width(filter_w)
                                .hint_text("Filtrar..."),
                        );
                        if resp.gained_focus() {
                            self.terminal.focused = false;
                        }
                    });
                });

                if let Some(mode) = self.new_item_dialog {
                    ui.horizontal(|ui| {
                        let hint = match mode {
                            NewItemMode::File => "ruta/archivo.rs",
                            NewItemMode::Directory => "nueva_carpeta",
                        };
                        let input_w = (panel_w - 68.0).max(60.0);
                        let resp = ui.add(
                            egui::TextEdit::singleline(&mut self.new_item_name)
                                .desired_width(input_w)
                                .hint_text(hint),
                        );
                        if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                            self.commit_new_item(mode);
                        }
                        if ui.small_button("OK").clicked() {
                            self.commit_new_item(mode);
                        }
                        if ui.small_button("x").clicked() {
                            self.new_item_dialog = None;
                        }
                    });
                }

                ui.separator();

                let filter = self.project.filter_query.trim().to_lowercase();
                let show_hidden = self.project.show_hidden;
                let mut file_to_open: Option<PathBuf> = None;

                egui::ScrollArea::both()
                    .id_salt("project_tree_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        render_tree_nodes(
                            ui,
                            &mut self.project.children,
                            0,
                            &filter,
                            show_hidden,
                            &mut self.project.watcher,
                            &self.editor,
                            &mut file_to_open,
                        );
                    });

                if let Some(path) = file_to_open {
                    self.editor.open_file(path);
                    self.terminal.focused = false;
                }
            });
    }

    fn commit_new_item(&mut self, mode: NewItemMode) {
        if self.new_item_name.trim().is_empty() {
            self.new_item_dialog = None;
            return;
        }
        match mode {
            NewItemMode::File => {
                if let Ok(path) = self.project.create_file(&self.new_item_name) {
                    self.editor.open_file(path);
                }
            }
            NewItemMode::Directory => {
                let _ = self.project.create_dir(&self.new_item_name);
            }
        }
        self.new_item_dialog = None;
        self.new_item_name.clear();
    }

    fn render_quick_open_modal(&mut self, ctx: &egui::Context) {
        if !self.quick_open_visible {
            return;
        }

        let mut close_modal = false;
        let mut chosen: Option<PathBuf> = None;

        egui::Window::new("Búsqueda rápida de archivos (Ctrl+P)")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_TOP, [0.0, 48.0])
            .default_width(480.0)
            .max_width(480.0)
            .show(ctx, |ui| {
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut self.quick_open_query)
                        .desired_width(f32::INFINITY)
                        .hint_text("Escribe el nombre del archivo..."),
                );
                resp.request_focus();

                if resp.changed() {
                    self.quick_open_results = self.project.find_files(&self.quick_open_query, 40);
                }

                if ui.input(|i| i.key_pressed(Key::Escape)) {
                    close_modal = true;
                }
                if ui.input(|i| i.key_pressed(Key::Enter)) {
                    if let Some(first) = self.quick_open_results.first() {
                        chosen = Some(first.clone());
                        close_modal = true;
                    }
                }

                ui.separator();
                egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                    for path in &self.quick_open_results {
                        let rel = path
                            .strip_prefix(&self.project.root)
                            .unwrap_or(path)
                            .display()
                            .to_string();
                        if ui.selectable_label(false, rel).clicked() {
                            chosen = Some(path.clone());
                            close_modal = true;
                        }
                    }
                });
            });

        if let Some(path) = chosen {
            self.editor.open_file(path);
        }
        if close_modal {
            self.quick_open_visible = false;
        }
    }
}

impl eframe::App for SuperCliApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_global_shortcuts(ctx);
        self.poll_filesystem_and_metrics();

        self.render_top_bar(ctx);
        self.render_status_bar(ctx);
        self.render_left_project_panel(ctx);

        // Render integrated terminal (Bottom or Right)
        if self.terminal.visible {
            let root = self.project.root.clone();
            match self.terminal.dock {
                TerminalDock::Bottom => {
                    egui::TopBottomPanel::bottom("terminal_panel")
                        .resizable(true)
                        .default_height(250.0)
                        .height_range(120.0..=600.0)
                        .show(ctx, |ui| {
                            render_terminal_panel(ui, &mut self.terminal, &root, ctx);
                        });
                }
                TerminalDock::Right => {
                    egui::SidePanel::right("terminal_panel_right")
                        .resizable(true)
                        .default_width(460.0)
                        .width_range(220.0..=900.0)
                        .show(ctx, |ui| {
                            ui.set_max_width(ui.available_width());
                            render_terminal_panel(ui, &mut self.terminal, &root, ctx);
                        });
                }
            }
        }

        // Central code editor panel
        let mut editor_focused = false;
        egui::CentralPanel::default().show(ctx, |ui| {
            render_editor_panel(ui, &mut self.editor, &mut editor_focused);
        });
        if editor_focused {
            self.terminal.focused = false;
        }

        self.render_quick_open_modal(ctx);
    }
}

#[allow(clippy::too_many_arguments)]
fn render_tree_nodes(
    ui: &mut egui::Ui,
    nodes: &mut [TreeNode],
    depth: usize,
    filter: &str,
    show_hidden: bool,
    watcher: &mut crate::project::watcher::ProjectWatcher,
    editor: &EditorManager,
    file_to_open: &mut Option<PathBuf>,
) {
    let indent = depth as f32 * 14.0;
    for node in nodes.iter_mut() {
        let matches_filter =
            filter.is_empty() || node.name.to_lowercase().contains(filter);

        if node.is_dir {
            if matches_filter || node.expanded {
                ui.horizontal(|ui| {
                    ui.add_space(indent);
                    let arrow = if node.expanded { "v" } else { ">" };
                    let label = RichText::new(format!("{arrow} {}", node.name))
                        .color(Color32::from_rgb(137, 180, 250));
                    if ui.selectable_label(false, label).clicked() {
                        ProjectTree::toggle_node_at(node, show_hidden, watcher);
                    }
                });
            }
            if node.expanded {
                render_tree_nodes(
                    ui,
                    &mut node.children,
                    depth + 1,
                    filter,
                    show_hidden,
                    watcher,
                    editor,
                    file_to_open,
                );
            }
        } else if matches_filter {
            ui.horizontal(|ui| {
                ui.add_space(indent + 12.0);
                let is_open = editor.is_open(&node.path);
                let label = RichText::new(&node.name).color(if is_open {
                    Color32::from_rgb(166, 227, 161)
                } else {
                    Color32::from_rgb(205, 214, 244)
                });
                if ui.selectable_label(is_open, label).clicked() {
                    *file_to_open = Some(node.path.clone());
                }
            });
        }
    }
}

fn apply_dark_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = Color32::from_rgb(24, 26, 34);
    visuals.window_fill = Color32::from_rgb(30, 32, 42);
    visuals.extreme_bg_color = Color32::from_rgb(18, 20, 26);
    visuals.selection.bg_fill = Color32::from_rgb(56, 72, 112);
    ctx.set_visuals(visuals);
}
