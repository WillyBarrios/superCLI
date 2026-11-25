use eframe::egui::{self, Color32, FontId, RichText};

use super::syntax::highlight_code;
use super::EditorManager;

pub fn render_editor_panel(ui: &mut egui::Ui, editor: &mut EditorManager, on_focus: &mut bool) {
    // Tab bar
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let mut close_idx = None;

        for (i, tab) in editor.tabs.iter().enumerate() {
            let selected = i == editor.active_tab;
            let dirty_mark = if tab.dirty { " *" } else { "" };
            let label = format!("{}{dirty_mark}", tab.title);

            if ui.selectable_label(selected, label).clicked() {
                editor.active_tab = i;
                *on_focus = true;
            }
            if ui.small_button("x").clicked() {
                close_idx = Some(i);
            }
        }

        if let Some(idx) = close_idx {
            editor.close_tab(idx);
        }

        if ui.small_button("+").on_hover_text("Nuevo archivo").clicked() {
            editor.new_untitled();
            *on_focus = true;
        }

        if let Some(msg) = &editor.status_message {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(msg).small().color(Color32::from_rgb(166, 227, 161)));
            });
        }
    });

    ui.separator();

    // Optional Find Bar (Ctrl+F)
    if editor.find_open {
        ui.horizontal(|ui| {
            ui.label("Buscar:");
            ui.add(
                egui::TextEdit::singleline(&mut editor.find_query)
                    .desired_width(220.0)
                    .hint_text("Texto a buscar..."),
            );
            if let Some(doc) = editor.active_doc() {
                if !editor.find_query.is_empty() {
                    let count = doc
                        .text_cache
                        .to_lowercase()
                        .matches(&editor.find_query.to_lowercase())
                        .count();
                    ui.label(RichText::new(format!("{count} coincidencias")).small().weak());
                }
            }
            if ui.small_button("x").clicked() {
                editor.find_open = false;
            }
        });
        ui.separator();
    }

    let font_size = editor.font_size;
    let Some(doc) = editor.active_doc_mut() else {
        ui.centered_and_justified(|ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.heading(RichText::new("superCLI").color(Color32::from_rgb(137, 180, 250)));
                ui.add_space(8.0);
                ui.label(
                    RichText::new(
                        "Editor nativo de bajo consumo RAM para trabajar con Agentes CLI",
                    )
                    .weak(),
                );
                ui.add_space(16.0);
                ui.label("• Selecciona un archivo en el explorador de la izquierda");
                ui.label("• Ctrl+P: Búsqueda rápida de archivos");
                ui.label("• Ctrl+B: Mostrar/ocultar explorador de proyecto");
                ui.label("• Ctrl+J: Mostrar/ocultar terminal integrada");
                ui.label("• Ctrl+S: Guardar archivo activo");
            });
        });
        return;
    };

    let ext = doc.extension().to_string();
    let line_count = doc.line_count();
    let prev_snapshot = doc.text_cache.clone();

    egui::ScrollArea::both()
        .id_salt("editor_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                // Line numbers gutter
                let mut gutter = String::with_capacity(line_count * 5);
                for n in 1..=line_count {
                    if n > 1 {
                        gutter.push('\n');
                    }
                    gutter.push_str(&format!("{n:>4} "));
                }

                ui.add(
                    egui::Label::new(
                        RichText::new(gutter)
                            .font(FontId::monospace(font_size))
                            .color(Color32::from_rgb(95, 102, 128)),
                    )
                    .selectable(false),
                );

                let mut layouter = move |ui: &egui::Ui, text: &str, _wrap_width: f32| {
                    let job = highlight_code(text, &ext, font_size);
                    ui.fonts(|f| f.layout_job(job))
                };

                let response = ui.add_sized(
                    ui.available_size(),
                    egui::TextEdit::multiline(&mut doc.text_cache)
                        .font(FontId::monospace(font_size))
                        .code_editor()
                        .lock_focus(true)
                        .frame(false)
                        .layouter(&mut layouter),
                );

                if response.gained_focus() || response.clicked() {
                    *on_focus = true;
                }
                if response.changed() {
                    doc.commit_edit(prev_snapshot);
                }
            });
        });
}
