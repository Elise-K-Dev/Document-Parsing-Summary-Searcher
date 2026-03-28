use std::error::Error;
use std::thread;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub db_path: String,
    pub staged_manifest: String,
    pub staged_files_root: String,
    pub http_bind: String,
    pub command: Command,
}

#[derive(Debug, Clone)]
pub enum Command {
    PrintConfig,
    Serve,
    InitDb,
    IngestManifest,
    IngestDocuments {
        family: Option<String>,
        limit: Option<usize>,
        jobs: Option<usize>,
        document_id: Option<i64>,
    },
    RebuildDocumentIndex,
    BuildIndex,
    Evaluate {
        gold_path: String,
    },
    Search {
        keyword: String,
        family: Option<String>,
        limit: usize,
    },
    Query {
        sql: String,
    },
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            db_path: default_data_path(&["backend", "evidence.db"]),
            staged_manifest: default_data_path(&["rag_stage", "manifests", "manifest.csv"]),
            staged_files_root: default_data_path(&["rag_stage", "files"]),
            http_bind: "127.0.0.1:8080".to_string(),
            command: Command::PrintConfig,
        }
    }
}

impl AppConfig {
    pub fn from_args(args: Vec<String>) -> Result<Self, Box<dyn Error>> {
        let mut config = Self::default();
        let mut i = 1usize;

        while i < args.len() {
            match args[i].as_str() {
                "--db" => {
                    i += 1;
                    config.db_path = args.get(i).ok_or("missing value after --db")?.clone();
                }
                "--manifest" => {
                    i += 1;
                    config.staged_manifest = args.get(i).ok_or("missing value after --manifest")?.clone();
                }
                "--files-root" => {
                    i += 1;
                    config.staged_files_root = args.get(i).ok_or("missing value after --files-root")?.clone();
                }
                "serve" => {
                    i += 1;
                    while i < args.len() {
                        match args[i].as_str() {
                            "--bind" => {
                                i += 1;
                                config.http_bind = args.get(i).ok_or("missing value after --bind")?.clone();
                            }
                            _ => {
                                i -= 1;
                                break;
                            }
                        }
                        i += 1;
                    }
                    config.command = Command::Serve;
                }
                "print-config" => config.command = Command::PrintConfig,
                "init-db" => config.command = Command::InitDb,
                "ingest-manifest" => config.command = Command::IngestManifest,
                "ingest-documents" | "ingest-xlsx" => {
                    let mut family = None;
                    let mut limit = None;
                    let mut jobs = None;
                    let mut document_id = None;
                    i += 1;
                    while i < args.len() {
                        match args[i].as_str() {
                            "--family" => {
                                i += 1;
                                family = Some(args.get(i).ok_or("missing value after --family")?.clone());
                            }
                            "--limit" => {
                                i += 1;
                                limit = Some(args.get(i).ok_or("missing value after --limit")?.parse()?);
                            }
                            "--jobs" => {
                                i += 1;
                                jobs = Some(args.get(i).ok_or("missing value after --jobs")?.parse()?);
                            }
                            "--document-id" => {
                                i += 1;
                                document_id = Some(args.get(i).ok_or("missing value after --document-id")?.parse()?);
                            }
                            _ => {
                                i -= 1;
                                break;
                            }
                        }
                        i += 1;
                    }
                    config.command = Command::IngestDocuments { family, limit, jobs, document_id };
                }
                "rebuild-document-index" => config.command = Command::RebuildDocumentIndex,
                "build-index" => config.command = Command::BuildIndex,
                "evaluate" => {
                    let default_gold = PathBuf::from("eval")
                        .join("evaluation_gold.csv")
                        .display()
                        .to_string();
                    let mut gold_path = default_gold;
                    i += 1;
                    while i < args.len() {
                        match args[i].as_str() {
                            "--gold" => {
                                i += 1;
                                gold_path = args.get(i).ok_or("missing value after --gold")?.clone();
                            }
                            _ => {
                                i -= 1;
                                break;
                            }
                        }
                        i += 1;
                    }
                    config.command = Command::Evaluate { gold_path };
                }
                "search" => {
                    let mut keyword = String::new();
                    let mut family = None;
                    let mut limit = 20usize;
                    i += 1;
                    while i < args.len() {
                        match args[i].as_str() {
                            "--keyword" => {
                                i += 1;
                                keyword = args.get(i).ok_or("missing value after --keyword")?.clone();
                            }
                            "--family" => {
                                i += 1;
                                family = Some(args.get(i).ok_or("missing value after --family")?.clone());
                            }
                            "--limit" => {
                                i += 1;
                                limit = args.get(i).ok_or("missing value after --limit")?.parse()?;
                            }
                            _ => {
                                i -= 1;
                                break;
                            }
                        }
                        i += 1;
                    }
                    if keyword.trim().is_empty() {
                        return Err("search requires --keyword".into());
                    }
                    config.command = Command::Search { keyword, family, limit };
                }
                "query" => {
                    let sql = args.get(i + 1).ok_or("query requires a SQL string argument")?.clone();
                    config.command = Command::Query { sql };
                    i += 1;
                }
                "--help" | "-h" => {
                    print_help();
                    std::process::exit(0);
                }
                other => return Err(format!("unknown argument: {other}").into()),
            }
            i += 1;
        }

        Ok(config)
    }
}

fn default_data_path(parts: &[&str]) -> String {
    let mut path = PathBuf::from("..");
    path.push("..");
    path.push("data");
    for part in parts {
        path.push(part);
    }
    path.display().to_string()
}

pub fn default_worker_count() -> usize {
    thread::available_parallelism()
        .map(|value| value.get())
        .unwrap_or(1)
}

fn print_help() {
    println!("evidence_backend");
    println!("Commands:");
    println!("  serve [--bind <host:port>]");
    println!("  print-config");
    println!("  init-db");
    println!("  ingest-manifest");
    println!("  ingest-documents [--family <family>] [--limit <n>] [--jobs <n>] [--document-id <id>]");
    println!("  ingest-xlsx [--family <family>] [--limit <n>] [--jobs <n>] [--document-id <id>]  # legacy alias");
    println!("  rebuild-document-index");
    println!("  build-index");
    println!("  evaluate [--gold <path>]");
    println!("  search --keyword <text> [--family <family>] [--limit <n>]");
    println!("  query \"<sql>\"");
    println!("Global options:");
    println!("  --db <path>");
    println!("  --manifest <path>");
    println!("  --files-root <path>");
    println!("Default ingest worker count:");
    println!("  {}", default_worker_count());
}
