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
            .with_inner_size([550.0, 400.0])
            .with_min_inner_size([400.0, 300.0]),
        ..Default::default()
    };

    let _ = eframe::run_native(
        "MKPatch Tools",
        options,
        Box::new(|_cc| Ok(Box::<MkPatchApp>::default())),
    );
}

impl eframe::App for MkPatchApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("🔧 MKPatch Tools");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let is_dark = ctx.style().visuals.dark_mode;
                    let text = if is_dark { "🌞 Claro" } else { "🌙 Escuro" };
                    if ui.button(text).clicked() {
                        if is_dark {
                            ctx.set_visuals(egui::Visuals::light());
                        } else {
                            ctx.set_visuals(egui::Visuals::dark());
                        }
                    }
                });
            });
            ui.separator();

            ui.add_space(10.0);
            ui.label("Embute o arquivo de configuração YML dentro do executável do patcher para ocultar URLs sensíveis.");
            ui.add_space(20.0);

            ui.label("Executável do Patcher (.exe):");
            ui.horizontal(|ui| {
                if ui.button("Selecionar EXE...").clicked() {
                    if let Some(path) = FileDialog::new()
                        .add_filter("Executáveis", &["exe"])
                        .pick_file()
                    {
                        self.exe_path = Some(path);
                        self.log.push_str("\n[Info] EXE Selecionado.");
                    }
                }
                if let Some(path) = &self.exe_path {
                    ui.label(path.file_name().unwrap_or_default().to_string_lossy());
                } else {
                    ui.label("Nenhum arquivo...");
                }
            });

            ui.add_space(15.0);

            ui.label("Arquivo de Configuração (.yml):");
            ui.horizontal(|ui| {
                if ui.button("Selecionar YML...").clicked() {
                    if let Some(path) = FileDialog::new()
                        .add_filter("YAML", &["yml", "yaml"])
                        .pick_file()
                    {
                        self.yml_path = Some(path);
                        self.log.push_str("\n[Info] YML Selecionado.");
                    }
                }
                if let Some(path) = &self.yml_path {
                    ui.label(path.file_name().unwrap_or_default().to_string_lossy());
                } else {
                    ui.label("Nenhum arquivo...");
                }
            });

            ui.add_space(25.0);

            if ui.button("🔒 Embutir Config no EXE").clicked() {
                if let (Some(exe), Some(yml)) = (&self.exe_path, &self.yml_path) {
                    let output_filename = exe
                        .file_stem()
                        .map(|s| format!("{}_embedded.exe", s.to_string_lossy()))
                        .unwrap_or_else(|| "output_embedded.exe".to_string());

                    let output_path = exe
                        .parent()
                        .map(|p| p.join(&output_filename))
                        .unwrap_or_else(|| PathBuf::from(&output_filename));

                    self.log.push_str("\n[Info] Processando...");

                    match embed_config_in_exe(exe, yml, &output_path) {
                        Ok(_) => {
                            self.log.push_str("\n[Sucesso] Config embutido com sucesso!");
                            self.log.push_str(&format!("\nSalvo em: {}", output_path.display()));
                        }
                        Err(e) => {
                            self.log.push_str(&format!("\n[Erro] {}", e));
                        }
                    }
                } else {
                    self.log.push_str("\n[Erro] Selecione o EXE e o YML primeiro.");
                }
            }

            ui.add_space(20.0);
            ui.separator();
            ui.label("Logs:");
            ui.add_space(5.0);

            let mut log_text = self.log.as_str();
            egui::ScrollArea::vertical()
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    ui.add_sized(
                        ui.available_size(),
                        egui::TextEdit::multiline(&mut log_text)
                            .interactive(false)
                            .font(egui::TextStyle::Monospace),
                    );
                });
        });
    }
}
