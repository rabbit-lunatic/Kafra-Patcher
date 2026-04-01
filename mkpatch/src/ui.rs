use std::path::PathBuf;
use eframe::egui;
use rfd::FileDialog;

use crate::embed::embed_config_in_exe;

pub struct MkPatchApp {
    exe_path: Option<PathBuf>,
    yml_path: Option<PathBuf>,
    log: String,
}

impl Default for MkPatchApp {
    fn default() -> Self {
        Self {
            exe_path: None,
            yml_path: None,
            log: "Pronto.".to_string(),
        }
    }
}

pub fn run_ui() {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([600.0, 520.0])
            .with_min_inner_size([500.0, 450.0])
            .with_title_shown(true),
        ..Default::default()
    };

    let _ = eframe::run_native(
        "MKPatch Tools",
        options,
        Box::new(|cc| {
            // Custom Visuals for a "Premium" look
            let mut visuals = egui::Visuals::dark();
            visuals.window_rounding = egui::Rounding::same(12.0);
            visuals.widgets.noninteractive.rounding = egui::Rounding::same(8.0);
            visuals.widgets.inactive.rounding = egui::Rounding::same(8.0);
            visuals.widgets.hovered.rounding = egui::Rounding::same(8.0);
            visuals.widgets.active.rounding = egui::Rounding::same(8.0);
            visuals.selection.bg_fill = egui::Color32::from_rgb(60, 120, 210);
            
            cc.egui_ctx.set_visuals(visuals);
            
            Box::<MkPatchApp>::default()
        }),
    );
}

fn ui_file_picker(
    ui: &mut egui::Ui,
    title: &str,
    path: &mut Option<PathBuf>,
    filter_name: &str,
    filter_ext: &[&str],
    icon: &str,
    log: &mut String,
) {
    ui.vertical(|ui| {
        ui.label(egui::RichText::new(title).strong().size(14.0));
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let btn_text = format!("{} Selecionar...", icon);
            let button = egui::Button::new(btn_text).min_size(egui::vec2(120.0, 32.0));
            
            if ui.add(button).clicked() {
                if let Some(p) = FileDialog::new()
                    .add_filter(filter_name, filter_ext)
                    .pick_file()
                {
                    *path = Some(p);
                    log.push_str(&format!("\n[Info] {} selecionado.", filter_name));
                }
            }

            ui.add_space(8.0);

            if let Some(p) = path {
                ui.label(egui::RichText::new("✅").color(egui::Color32::GREEN));
                ui.label(egui::RichText::new(p.file_name().unwrap_or_default().to_string_lossy())
                    .italics()
                    .color(ui.visuals().strong_text_color()));
            } else {
                ui.label(egui::RichText::new("⭕").color(egui::Color32::from_rgb(200, 100, 100)));
                ui.label(egui::RichText::new("Nenhum arquivo...").weak());
            }
        });
    });
}

impl eframe::App for MkPatchApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            // Header
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.heading(egui::RichText::new("🔧 MKPatch Tools").strong().size(24.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let is_dark = ctx.style().visuals.dark_mode;
                    if ui.button(if is_dark { "🌞" } else { "🌙" }).on_hover_text("Alternar Tema").clicked() {
                        if is_dark {
                            ctx.set_visuals(egui::Visuals::light());
                        } else {
                            ctx.set_visuals(egui::Visuals::dark());
                        }
                    }
                });
            });
            ui.add_space(5.0);
            ui.separator();
            ui.add_space(15.0);

            // Settings Description
            ui.label(egui::RichText::new("Embute o arquivo de configuração YML dentro do executável para ocultar URLs e configurações sensíveis.")
                .weak());
            ui.add_space(15.0);

            // Configuration Group (Card)
            egui::Frame::group(ui.style())
                .fill(ui.visuals().faint_bg_color)
                .rounding(10.0)
                .inner_margin(15.0)
                .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    
                    ui_file_picker(
                        ui,
                        "Executável do Patcher (.exe):",
                        &mut self.exe_path,
                        "Executáveis",
                        &["exe"],
                        "📁",
                        &mut self.log,
                    );
                    
                    ui.add_space(20.0);

                    ui_file_picker(
                        ui,
                        "Arquivo de Configuração (.yml):",
                        &mut self.yml_path,
                        "YAML",
                        &["yml", "yaml"],
                        "📄",
                        &mut self.log,
                    );
                });

            ui.add_space(30.0);

            // Action Section
            ui.vertical_centered(|ui| {
                let ready = self.exe_path.is_some() && self.yml_path.is_some();
                
                let btn_text = egui::RichText::new("🔒 Embutir Config no EXE")
                    .size(18.0)
                    .strong()
                    .color(egui::Color32::WHITE);

                let btn = egui::Button::new(btn_text)
                    .min_size(egui::vec2(280.0, 48.0))
                    .rounding(12.0)
                    .fill(if ready { egui::Color32::from_rgb(60, 120, 210) } else { ui.visuals().widgets.inactive.bg_fill });

                let mut response = ui.add_enabled(ready, btn);
                
                if !ready {
                    response = response.on_hover_text("Selecione ambos os arquivos para continuar.");
                }

                if response.clicked() {
                    if let (Some(exe), Some(yml)) = (self.exe_path.as_ref(), self.yml_path.as_ref()) {
                        let output_filename = exe
                            .file_stem()
                            .map(|s| format!("{}_embedded.exe", s.to_string_lossy()))
                            .unwrap_or_else(|| "output_embedded.exe".to_string());

                        let output_path = exe
                            .parent()
                            .map(|p| p.join(&output_filename))
                            .unwrap_or_else(|| PathBuf::from(&output_filename));

                        self.log.push_str("\n[Info] Iniciando processamento...");

                        match embed_config_in_exe(exe, yml, &output_path) {
                            Ok(_) => {
                                self.log.push_str("\n[Sucesso] Configuração embutida com sucesso!");
                                self.log.push_str(&format!("\n[Sucesso] Salvo em: {}", output_path.display()));
                            }
                            Err(e) => {
                                self.log.push_str(&format!("\n[Erro] Falha ao embutir: {}", e));
                            }
                        }
                    }
                }
            });

            ui.add_space(25.0);

            // Log Section (Terminal Style)
            ui.label(egui::RichText::new("LOG DE ATIVIDADES").small().strong().color(ui.visuals().weak_text_color()));
            ui.add_space(4.0);
            
            egui::Frame::canvas(ui.style())
                .fill(egui::Color32::from_black_alpha(100))
                .rounding(6.0)
                .inner_margin(8.0)
                .show(ui, |ui| {
                    let mut log_text = self.log.as_str();
                    egui::ScrollArea::vertical()
                        .stick_to_bottom(true)
                        .max_height(120.0)
                        .show(ui, |ui| {
                            ui.add_sized(
                                [ui.available_width(), 120.0],
                                egui::TextEdit::multiline(&mut log_text)
                                    .interactive(false)
                                    .font(egui::TextStyle::Monospace)
                                    .frame(false)
                                    .text_color(egui::Color32::from_rgb(200, 200, 200)),
                            );
                        });
                });
            
            ui.add_space(10.0);
        });
    }
}
