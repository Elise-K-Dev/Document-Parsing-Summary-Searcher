mod app;
mod config;
mod csv_util;
mod db;
mod ooxml_ingest;
mod web_api;
mod xlsx_ingest;

use std::env;
use std::error::Error;

use config::{AppConfig, Command};

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let config = AppConfig::from_args(env::args().collect())?;
    match config.command.clone() {
        Command::PrintConfig => app::print_boot_message(&config),
        Command::Serve => app::serve(&config)?,
        Command::InitDb => app::init_db(&config)?,
        Command::IngestManifest => app::ingest_manifest(&config)?,
        Command::IngestDocuments { family, limit, jobs, document_id } => {
            app::ingest_xlsx(&config, family.as_deref(), limit, jobs, document_id)?
        }
        Command::RebuildDocumentIndex => app::rebuild_document_index(&config)?,
        Command::BuildIndex => {
            app::init_db(&config)?;
            app::ingest_manifest(&config)?;
            app::ingest_xlsx(&config, None, None, None, None)?;
        }
        Command::Evaluate { gold_path } => app::evaluate(&config, &gold_path)?,
        Command::Search { keyword, family, limit } => {
            app::search(&config, &keyword, family.as_deref(), limit)?
        }
        Command::Query { sql } => app::run_query(&config, &sql)?,
    }
    Ok(())
}
