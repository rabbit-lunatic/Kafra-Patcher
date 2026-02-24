pub mod embed;
mod generator;
mod patch_definition;
mod ui;

use std::path::PathBuf;
use std::{env, process};

use anyhow::{anyhow, Context, Result};
use generator::generate_patch_from_definition;
use log::LevelFilter;
use patch_definition::parse_patch_definition;
use simple_logger::SimpleLogger;
use structopt::StructOpt; // Import from new module

const PKG_NAME: &str = env!("CARGO_PKG_NAME");
const PKG_AUTHORS: &str = env!("CARGO_PKG_AUTHORS");
const PKG_DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");

#[derive(Debug, StructOpt)]
#[structopt(name = PKG_NAME, about = PKG_DESCRIPTION, author = PKG_AUTHORS)]
struct Opt {
    #[structopt(short, long, help = "Enable verbose logging")]
    verbose: bool,
    #[structopt(parse(from_os_str), help = "Path to a patch definition file")]
    patch_definition_file: PathBuf,
    #[structopt(
        parse(from_os_str),
        short,
        long,
        help = "Path to the directory that contains patch data (default: current working directory)"
    )]
    patch_data_directory: Option<PathBuf>,
    #[structopt(
        parse(from_os_str),
        short,
        long,
        help = "Path to the output archive (default: <patch_definition_file_name>.thor)"
    )]
    output_file: Option<PathBuf>,
}

fn run(cli_args: Opt) -> Result<()> {
    let patch_data_directory = cli_args
        .patch_data_directory
        .unwrap_or_else(|| PathBuf::from("."));
    let output_file_path = cli_args.output_file.unwrap_or(PathBuf::from(
        cli_args
            .patch_definition_file
            .with_extension("thor")
            .file_name()
            .ok_or_else(|| anyhow!("Invalid patch definition file name"))?,
    ));

    // Parse the YAML definition file
    log::info!(
        "Processing '{}'",
        cli_args.patch_definition_file.to_string_lossy()
    );
    let patch_definition = parse_patch_definition(&cli_args.patch_definition_file)
        .context("Failed to parse the patch definition")?;

    // Display patch info
    log::info!("GRF merging: {}", patch_definition.use_grf_merging);
    log::info!("Checksums included: {}", patch_definition.include_checksums);
    if let Some(target_grf_name) = &patch_definition.target_grf_name {
        log::info!("Target GRF: '{}'", target_grf_name);
    } else {
        log::info!("Target: Game directory");
    }

    // Generate THOR archive
    generate_patch_from_definition(patch_definition, patch_data_directory, &output_file_path)
        .context("Failed to generate patch from definition")?;
    log::info!(
        "Patch generated at '{}'",
        output_file_path.to_string_lossy()
    );
    Ok(())
}

// Shim for missing symbol in MinGW linking of tinyfiledialogs
// This satisfies the linker if shcore library is not found or processed correctly.
#[cfg(all(windows, target_env = "gnu"))]
#[no_mangle]
pub extern "system" fn SetProcessDpiAwareness(_: i32) -> i32 {
    // E_NOTIMPL = 0x80004001
    -2147467263
}

fn main() {
    const SUCCESS_EXIT_CODE: i32 = 0;
    const FAILURE_EXIT_CODE: i32 = 1;

    // Check if we should run in UI mode
    // If no arguments provided interactively (length 1, just the executable), run UI
    let args: Vec<String> = env::args().collect();
    if args.len() <= 1 {
        // Run UI
        // Initialize logger for UI? Maybe not needed or to file?
        // We will just print to stdout for now if needed.
        ui::run_ui();
        return;
    }

    // Parse CLI arguments
    let cli_args = Opt::from_args();
    // Initialize the logger
    init_logger(cli_args.verbose).expect("Failed to initalize the logger");

    // Run the actual program
    let result = run(cli_args);
    match result {
        Ok(()) => {
            process::exit(SUCCESS_EXIT_CODE);
        }
        Err(err) => {
            log::error!("{:#}", err);
            process::exit(FAILURE_EXIT_CODE);
        }
    }
}

fn init_logger(verbose: bool) -> Result<()> {
    let level_filter = if verbose {
        LevelFilter::Trace
    } else {
        LevelFilter::Info
    };

    SimpleLogger::new()
        .with_level(LevelFilter::Off)
        .with_module_level(PKG_NAME, level_filter)
        .init()?;
    Ok(())
}
