use serde::Deserialize;
use std::path::{Path, PathBuf};
use tao::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use tinyfiledialogs as tfd;
use wry::webview::{WebView, WebViewBuilder};

use crate::embed::embed_config_in_exe;
use crate::generator::generate_patch_from_definition;
use crate::patch_definition::{PatchDefinition, PatchEntry};

enum UiEvent {
    SelectFiles,
    SelectExe,
    SelectYml,
    Generate(String),
    Embed(String),
}

pub fn run_ui() {
    let event_loop = EventLoop::<UiEvent>::with_user_event();
    let proxy = event_loop.create_proxy();

    let window = WindowBuilder::new()
        .with_title("MKPatch Tools")
        .with_inner_size(LogicalSize::new(600.0, 700.0))
        .with_resizable(true)
        .build(&event_loop)
        .unwrap();

    let html_content = include_str!("assets/index.html");

    let handler_proxy = proxy.clone();
    let handler = move |_: &tao::window::Window, req: String| {
        if req == "select_files" {
            let _ = handler_proxy.send_event(UiEvent::SelectFiles);
        } else if req == "select_exe" {
            let _ = handler_proxy.send_event(UiEvent::SelectExe);
        } else if req == "select_yml" {
            let _ = handler_proxy.send_event(UiEvent::SelectYml);
        } else if req.starts_with("generate:") {
            let json_str = req[9..].to_string();
            let _ = handler_proxy.send_event(UiEvent::Generate(json_str));
        } else if req.starts_with("embed:") {
            let json_str = req[6..].to_string();
            let _ = handler_proxy.send_event(UiEvent::Embed(json_str));
        }
    };

    let webview = WebViewBuilder::new(window)
        .unwrap()
        .with_html(html_content)
        .unwrap()
        .with_initialization_script(
            "window.external = { invoke: function(s) { window.ipc.postMessage(s); } };",
        )
        .with_ipc_handler(handler)
        .build()
        .unwrap();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::UserEvent(ui_event) => match ui_event {
                UiEvent::SelectFiles => {
                    handle_select_files(&webview);
                }
                UiEvent::SelectExe => {
                    handle_select_exe(&webview);
                }
                UiEvent::SelectYml => {
                    handle_select_yml(&webview);
                }
                UiEvent::Generate(json_str) => {
                    handle_generate(&webview, &json_str);
                }
                UiEvent::Embed(json_str) => {
                    handle_embed(&webview, &json_str);
                }
            },
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                *control_flow = ControlFlow::Exit;
            }
            _ => (),
        }
    });
}

fn handle_select_files(webview: &WebView) {
    let files = tfd::open_file_dialog_multi("Select Files to Patch", "", None);

    if let Some(files) = files {
        let files_json = serde_json::to_string(&files).unwrap_or("[]".to_string());
        let js = format!("filesSelected({})", files_json);
        let _ = webview.evaluate_script(&js);
    }
}

fn handle_select_exe(webview: &WebView) {
    let file = tfd::open_file_dialog(
        "Selecionar Executável do Patcher",
        "",
        Some((&["*.exe"], "Executáveis (*.exe)")),
    );

    if let Some(path) = file {
        let path_escaped = path.replace('\\', "\\\\").replace('"', "\\\"");
        let js = format!("exeSelected(\"{}\")", path_escaped);
        let _ = webview.evaluate_script(&js);
    }
}

fn handle_select_yml(webview: &WebView) {
    let file = tfd::open_file_dialog(
        "Selecionar Arquivo de Configuração",
        "",
        Some((&["*.yml", "*.yaml"], "Arquivos YAML (*.yml, *.yaml)")),
    );

    if let Some(path) = file {
        let path_escaped = path.replace('\\', "\\\\").replace('"', "\\\"");
        let js = format!("ymlSelected(\"{}\")", path_escaped);
        let _ = webview.evaluate_script(&js);
    }
}

#[derive(Deserialize)]
struct EmbedInput {
    exe_path: String,
    yml_path: String,
}

fn handle_embed(webview: &WebView, json_str: &str) {
    let input: EmbedInput = match serde_json::from_str(json_str) {
        Ok(i) => i,
        Err(e) => {
            let _ =
                webview.evaluate_script(&format!("logMessage('Erro ao processar input: {}')", e));
            return;
        }
    };

    let exe_path = PathBuf::from(&input.exe_path);
    let yml_path = PathBuf::from(&input.yml_path);

    // Gerar nome do arquivo de saída
    let output_filename = exe_path
        .file_stem()
        .map(|s| format!("{}_embedded.exe", s.to_string_lossy()))
        .unwrap_or_else(|| "output_embedded.exe".to_string());

    let output_path = exe_path
        .parent()
        .map(|p| p.join(&output_filename))
        .unwrap_or_else(|| PathBuf::from(&output_filename));

    let _ = webview.evaluate_script("logMessage('Processando...')");

    match embed_config_in_exe(&exe_path, &yml_path, &output_path) {
        Ok(_) => {
            let output_display = output_path.display().to_string().replace('\\', "\\\\");
            let _ = webview.evaluate_script(&format!(
                "logMessage('✅ Sucesso! Arquivo gerado: {}')",
                output_display
            ));
            let _ = webview.evaluate_script(&format!(
                "alert('Config embutido com sucesso!\\n\\nArquivo gerado:\\n{}')",
                output_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
            ));
        }
        Err(e) => {
            let error_msg = format!("{:#}", e).replace('"', "'");
            let _ = webview.evaluate_script(&format!("logMessage('❌ Erro: {}')", error_msg));
            let _ = webview
                .evaluate_script(&format!("alert('Erro ao embutir config:\\n{}')", error_msg));
        }
    }
}

#[derive(Deserialize)]
struct UIInput {
    target_grf: String,
    output_filename: String,
    merge_grf: bool,
    files: Vec<String>,
}

fn handle_generate(webview: &WebView, json_str: &str) {
    let input: UIInput = match serde_json::from_str(json_str) {
        Ok(i) => i,
        Err(e) => {
            let _ = webview.evaluate_script(&format!("logMessage('Error parsing input: {}')", e));
            return;
        }
    };

    if input.files.is_empty() {
        let _ = webview.evaluate_script("logMessage('No files selected!')");
        return;
    }

    let entries_mapped: Vec<PatchEntry> = input
        .files
        .iter()
        .map(|f| {
            let path = Path::new(f);
            let filename = path.file_name().unwrap().to_string_lossy().to_string();
            PatchEntry {
                relative_path: f.clone(),
                is_removed: false,
                in_grf_path: Some(filename),
            }
        })
        .collect();

    let def_for_gen = PatchDefinition {
        include_checksums: true,
        use_grf_merging: input.merge_grf,
        target_grf_name: if input.target_grf.is_empty() {
            None
        } else {
            Some(input.target_grf)
        },
        entries: entries_mapped,
    };

    let output_path = PathBuf::from(&input.output_filename);
    let output_path = if output_path.extension().is_none() {
        output_path.with_extension("thor")
    } else {
        output_path
    };

    let _ = webview.evaluate_script("logMessage('Generating patch...')");

    match generate_patch_from_definition(def_for_gen, ".", &output_path) {
        Ok(_) => {
            let output_display = output_path.display().to_string().replace("\\", "\\\\");
            let _ = webview.evaluate_script(&format!(
                "logMessage('Success! Patch saved to: {}')",
                output_display
            ));
            let _ = webview.evaluate_script("alert('Patch Generated Successfully!')");
        }
        Err(e) => {
            let _ = webview.evaluate_script(&format!("logMessage('Error: {}')", e));
        }
    }
}
