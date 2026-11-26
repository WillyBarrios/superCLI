use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, Key, RichText, Sense};

use crate::editor::view::render_editor_panel;
use crate::editor::EditorManager;
use crate::platform::memory::current_ram_mb;
use crate::platform::shell::{default_shell, open_file_location};
use crate::platform::updater::{OtaUpdater, UpdateStatus, CURRENT_VERSION};
use crate::project::{ProjectTree, TreeNode};
use crate::terminal::view::render_terminal_panel;
use crate::terminal::{TerminalDock, TerminalManager};

pub struct SuperCliApp {
    project: ProjectTree,
    editor: EditorManager,
    terminal: TerminalManager,
    updater: OtaUpdater,
    left_panel_open: bool,
    quick_open_visible: bool,
    quick_open_query: String,
    quick_open_results: Vec<PathBuf>,
    new_item_dialog: Option<NewItemMode>,
    new_item_name: String,
    rename_target: Option<PathBuf>,
    rename_input: String,
    view_path_target: Option<PathBuf>,
    cached_ram_mb: f64,
    last_ram_sample: Instant,
    shell_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NewItemMode {
    File,
    Directory,
}

#[derive(Debug, Clone)]
enum TreeContextAction {
    Rename(PathBuf),
    ViewPath(PathBuf),
    CopyPath(PathBuf),
    OpenLocation(PathBuf),
}

impl SuperCliApp {
    pub fn new(cc: &eframe::CreationContext<'_>, root_dir: PathBuf) -> Self {
        apply_dark_theme(&cc.egui_ctx);
        let shell = default_shell();
        let updater = OtaUpdater::new(cc.egui_ctx.clone());
        Self {
            project: ProjectTree::new(root_dir),
            editor: EditorManager::new(),
            terminal: TerminalManager::new(),
            updater,
            left_panel_open: true,
            quick_open_visible: false,
            quick_open_query: String::new(),
            quick_open_results: Vec::new(),
            new_item_dialog: None,
            new_item_name: String::new(),
            rename_target: None,
            rename_input: String::new(),
            view_path_target: None,
            cached_ram_mb: current_ram_mb(),
            last_ram_sample: Instant::now(),
            shell_name: shell.display_name,
        }
    }

    fn is_update_blocking(&self, status: &UpdateStatus) -> bool {
        match status {
            UpdateStatus::Available(info)
            | UpdateStatus::Downloading(info)
            | UpdateStatus::Error(info, _) => info.is_mandatory,
            UpdateStatus::ReadyToRestart(_) | UpdateStatus::Idle => false,
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

    fn render_top_bar(&mut self, ctx: &egui::Context, update_status: &UpdateStatus) {
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

                    // Non-intrusive OTA update notification in the top bar
                    match update_status {
                        UpdateStatus::Available(info) if !info.is_mandatory => {
                            ui.separator();
                            ui.label(
                                RichText::new(format!("Nueva {}", info.version_tag))
                                    .small()
                                    .color(Color32::from_rgb(166, 227, 161)),
                            );
                            if ui
                                .small_button("Instalar actualización")
                                .on_hover_text("Descargar e instalar la nueva versión")
                                .clicked()
                            {
                                self.updater.start_install(info.clone(), ctx.clone());
                            }
                        }
                        UpdateStatus::Downloading(info) if !info.is_mandatory => {
                            ui.separator();
                            ui.label(
                                RichText::new(format!("Descargando {}...", info.version_tag))
                                    .small()
                                    .color(Color32::from_rgb(249, 226, 175)),
                            );
                        }
                        UpdateStatus::ReadyToRestart(msg) => {
                            ui.separator();
                            ui.label(
                                RichText::new(msg)
                                    .small()
                                    .color(Color32::from_rgb(166, 227, 161)),
                            );
                        }
                        UpdateStatus::Error(info, err) if !info.is_mandatory => {
                            ui.separator();
                            ui.label(
                                RichText::new(err)
                                    .small()
                                    .color(Color32::from_rgb(243, 139, 168)),
                            );
                            if ui.small_button("Reintentar").clicked() {
                                self.updater.start_install(info.clone(), ctx.clone());
                            }
                        }
                        _ => {}
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
                    ui.label(RichText::new(format!("v{CURRENT_VERSION}")).small().weak());
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
                let mut ctx_action: Option<TreeContextAction> = None;

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
                            &mut ctx_action,
                        );
                    });

                if let Some(path) = file_to_open {
                    self.editor.open_file(path);
                    self.terminal.focused = false;
                }

                if let Some(action) = ctx_action {
                    self.handle_tree_context_action(action, ctx);
                }
            });
    }

    fn handle_tree_context_action(&mut self, action: TreeContextAction, ctx: &egui::Context) {
        match action {
            TreeContextAction::Rename(path) => {
                let current_name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();
                self.rename_input = current_name;
                self.rename_target = Some(path);
                self.terminal.focused = false;
            }
            TreeContextAction::ViewPath(path) => {
                self.view_path_target = Some(path);
                self.terminal.focused = false;
            }
            TreeContextAction::CopyPath(path) => {
                let full = path.display().to_string();
                ctx.copy_text(full);
                self.editor.status_message = Some("Ruta copiada al portapapeles".to_string());
            }
            TreeContextAction::OpenLocation(path) => {
                open_file_location(&path);
            }
        }
    }

    fn render_file_modals(&mut self, ctx: &egui::Context) {
        // Rename modal
        if let Some(target_path) = self.rename_target.clone() {
            let mut close = false;
            let mut confirm = false;
            egui::Window::new("Renombrar")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .default_width(360.0)
                .show(ctx, |ui| {
                    ui.label(
                        RichText::new(target_path.display().to_string())
                            .small()
                            .weak(),
                    );
                    ui.add_space(6.0);
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut self.rename_input)
                            .desired_width(f32::INFINITY),
                    );
                    resp.request_focus();
                    if ui.input(|i| i.key_pressed(Key::Enter)) {
                        confirm = true;
                    }
                    if ui.input(|i| i.key_pressed(Key::Escape)) {
                        close = true;
                    }
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Renombrar").clicked() {
                            confirm = true;
                        }
                        if ui.button("Cancelar").clicked() {
                            close = true;
                        }
                    });
                });

            if confirm {
                match self.project.rename_path(&target_path, &self.rename_input) {
                    Ok(new_path) => {
                        self.editor.notify_renamed(&target_path, &new_path);
                        self.editor.status_message = Some(format!(
                            "Renombrado a {}",
                            new_path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("")
                        ));
                    }
                    Err(err) => {
                        self.editor.status_message = Some(format!("Error al renombrar: {err}"));
                    }
                }
                close = true;
            }
            if close {
                self.rename_target = None;
                self.rename_input.clear();
            }
        }

        // View full path modal
        if let Some(path) = self.view_path_target.clone() {
            let mut close = false;
            let mut path_str = path.display().to_string();
            egui::Window::new("Ruta del archivo")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .default_width(480.0)
                .show(ctx, |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut path_str)
                            .desired_width(f32::INFINITY)
                            .interactive(true),
                    );
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Copiar path").clicked() {
                            ctx.copy_text(path.display().to_string());
                            self.editor.status_message =
                                Some("Ruta copiada al portapapeles".to_string());
                            close = true;
                        }
                        if ui.button("Abrir ubicación").clicked() {
                            open_file_location(&path);
                            close = true;
                        }
                        if ui.button("Cerrar").clicked() {
                            close = true;
                        }
                    });
                });
            if close || ctx.input(|i| i.key_pressed(Key::Escape)) {
                self.view_path_target = None;
            }
        }
    }

    fn render_mandatory_update_blocker(&mut self, ctx: &egui::Context, status: &UpdateStatus) {
        let screen_rect = ctx.screen_rect();
        // Full-screen translucent overlay that blocks interaction with the editor & terminal
        egui::Area::new(egui::Id::new("mandatory_update_scrim"))
            .fixed_pos(screen_rect.min)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let _ = ui.allocate_rect(screen_rect, Sense::click_and_drag());
                ui.painter().rect_filled(
                    screen_rect,
                    0.0,
                    Color32::from_rgba_unmultiplied(10, 12, 18, 215),
                );
            });

        egui::Window::new("Actualización obligatoria requerida")
            .collapsible(false)
            .resizable(false)
            .order(egui::Order::Tooltip)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .default_width(460.0)
            .show(ctx, |ui| {
                match status {
                    UpdateStatus::Available(info) => {
                        ui.label(
                            RichText::new(format!(
                                "Tu versión actual (v{CURRENT_VERSION}) está 2 o más versiones mayores por detrás de {}.",
                                info.version_tag
                            ))
                            .strong(),
                        );
                        ui.add_space(6.0);
                        ui.label(
                            "Para continuar trabajando en superCLI es necesario instalar la actualización. El instalador reemplazará automáticamente la versión anterior.",
                        );
                        ui.add_space(12.0);
                        if ui
                            .button(format!("Descargar e instalar {}", info.version_tag))
                            .clicked()
                        {
                            self.updater.start_install(info.clone(), ctx.clone());
                        }
                    }
                    UpdateStatus::Downloading(info) => {
                        ui.label(
                            RichText::new(format!("Descargando {}...", info.version_tag))
                                .strong()
                                .color(Color32::from_rgb(249, 226, 175)),
                        );
                        ui.add_space(6.0);
                        ui.label("Al finalizar se abrirá el instalador automáticamente.");
                    }
                    UpdateStatus::Error(info, err) => {
                        ui.label(
                            RichText::new(format!("Error al actualizar a {}:", info.version_tag))
                                .color(Color32::from_rgb(243, 139, 168))
                                .strong(),
                        );
                        ui.label(err);
                        ui.add_space(10.0);
                        if ui.button("Reintentar descarga").clicked() {
                            self.updater.start_install(info.clone(), ctx.clone());
                        }
                    }
                    _ => {}
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
        let update_status = self.updater.snapshot();
        let blocked = self.is_update_blocking(&update_status);

        if !blocked {
            self.handle_global_shortcuts(ctx);
        } else {
            self.terminal.focused = false;
        }
        self.poll_filesystem_and_metrics();

        self.render_top_bar(ctx, &update_status);
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
        if editor_focused && !blocked {
            self.terminal.focused = false;
        }

        if blocked {
            self.render_mandatory_update_blocker(ctx, &update_status);
        } else {
            self.render_quick_open_modal(ctx);
            self.render_file_modals(ctx);
        }
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
    ctx_action: &mut Option<TreeContextAction>,
) {
    let indent = depth as f32 * 14.0;
    for node in nodes.iter_mut() {
        let matches_filter = filter.is_empty() || node.name.to_lowercase().contains(filter);

        if node.is_dir {
            if matches_filter || node.expanded {
                ui.horizontal(|ui| {
                    ui.add_space(indent);
                    let arrow = if node.expanded { "v" } else { ">" };
                    let label = RichText::new(format!("{arrow} {}", node.name))
                        .color(Color32::from_rgb(137, 180, 250));
                    let resp = ui.selectable_label(false, label);
                    if resp.clicked() {
                        ProjectTree::toggle_node_at(node, show_hidden, watcher);
                    }
                    let path_clone = node.path.clone();
                    resp.context_menu(|ui| {
                        render_item_context_menu(ui, &path_clone, ctx_action);
                    });
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
                    ctx_action,
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
                let resp = ui.selectable_label(is_open, label);
                if resp.clicked() {
                    *file_to_open = Some(node.path.clone());
                }
                let path_clone = node.path.clone();
                resp.context_menu(|ui| {
                    render_item_context_menu(ui, &path_clone, ctx_action);
                });
            });
        }
    }
}

fn render_item_context_menu(
    ui: &mut egui::Ui,
    path: &PathBuf,
    ctx_action: &mut Option<TreeContextAction>,
) {
    if ui.button("Renombrar").clicked() {
        *ctx_action = Some(TreeContextAction::Rename(path.clone()));
        ui.close_menu();
    }
    if ui.button("Ver path").clicked() {
        *ctx_action = Some(TreeContextAction::ViewPath(path.clone()));
        ui.close_menu();
    }
    if ui.button("Copiar path").clicked() {
        *ctx_action = Some(TreeContextAction::CopyPath(path.clone()));
        ui.close_menu();
    }
    ui.separator();
    if ui.button("Abrir ubicación del archivo").clicked() {
        *ctx_action = Some(TreeContextAction::OpenLocation(path.clone()));
        ui.close_menu();
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
