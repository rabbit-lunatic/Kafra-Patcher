#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

pub mod embed;
mod ui;

use std::env;
use anyhow::Result;
use log::LevelFilter;
use simple_logger::SimpleLogger;

const PKG_NAME: &str = env!("CARGO_PKG_NAME");

fn main() {
    // Initialize the logger
    init_logger(false).expect("Failed to initalize the logger");

    // Run UI
    ui::run_ui();
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
