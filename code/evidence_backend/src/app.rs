use std::error::Error;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::Duration;

use crate::config::{AppConfig, default_worker_count};
use crate::{csv_util, db, ooxml_ingest, web_api, xlsx_ingest};

pub fn print_boot_message(config: &AppConfig) {
    println!("evidence_backend bootstrap");
    println!("db_path={}", config.db_path);
    println!("staged_manifest={}", config.staged_manifest);
    println!("staged_files_root={}", config.staged_files_root);
    println!("http_bind={}", config.http_bind);
}

pub fn serve(config: &AppConfig) -> Result<(), Box<dyn Error>> {
    db::ensure_parent_dir(&config.db_path)?;
    let conn = db::open_connection(&config.db_path)?;
    db::apply_schema(&conn)?;
    drop(conn);
    web_api::serve(config)
}

pub fn init_db(config: &AppConfig) -> Result<(), Box<dyn Error>> {
    db::ensure_parent_dir(&config.db_path)?;
    let conn = db::open_connection(&config.db_path)?;
    db::apply_schema(&conn)?;
    println!("init-db complete");
    println!("db_path={}", config.db_path);
    Ok(())
}

pub fn ingest_manifest(config: &AppConfig) -> Result<(), Box<dyn Error>> {
    let conn = db::open_connection(&config.db_path)?;
    db::apply_schema(&conn)?;
    let records = csv_util::read_csv_records(&config.staged_manifest)?;
    let inserted = db::upsert_documents(&conn, &records)?;
    let synced = db::sync_all_document_profiles(&conn)?;
    println!("ingest-manifest complete");
    println!("manifest={}", config.staged_manifest);
    println!("rows={}", records.len());
    println!("upserted={inserted}");
    println!("document_profiles_synced={synced}");
    Ok(())
}

pub fn ingest_xlsx(
    config: &AppConfig,
    family: Option<&str>,
    limit: Option<usize>,
    jobs: Option<usize>,
    document_id: Option<i64>,
) -> Result<(), Box<dyn Error>> {
    let conn = db::open_connection(&config.db_path)?;
    db::apply_schema(&conn)?;
    let documents = db::list_ingestible_documents(&conn, family, limit, document_id)?;
    drop(conn);

    let requested_jobs = jobs.unwrap_or_else(default_worker_count);
    let worker_count = requested_jobs.max(1).min(documents.len().max(1));
    let documents = Arc::new(documents);
    let cursor = Arc::new(AtomicUsize::new(0));
    let (tx, rx) = mpsc::channel::<WorkerMessage>();
    thread::scope(|scope| {
        let mut handles = Vec::with_capacity(worker_count);
        for worker_id in 0..worker_count {
            let tx = tx.clone();
            let documents = Arc::clone(&documents);
            let cursor = Arc::clone(&cursor);
            let db_path = config.db_path.clone();
            let handle = scope.spawn(move || -> Result<(), String> {
                let mut conn = db::open_connection(&db_path).map_err(|err| err.to_string())?;
                db::apply_schema(&conn).map_err(|err| err.to_string())?;

                loop {
                    let index = cursor.fetch_add(1, Ordering::SeqCst);
                    if index >= documents.len() {
                        break;
                    }
                    let document = &documents[index];
                    let outcome = process_document_with_retry(&mut conn, document);
                    let message = match outcome {
                        Ok((section_count, row_count)) => {
                            retry_db_action(|| db::mark_document_parsed(&conn, document.id))
                                .map_err(|err| err.to_string())?;
                            retry_db_action(|| db::sync_document_profile(&conn, document.id))
                                .map_err(|err| err.to_string())?;
                            WorkerMessage::Success {
                                worker_id,
                                document_id: document.id,
                                path: document.staged_path.clone(),
                                section_count,
                                row_count,
                            }
                        }
                        Err(err) => {
                            let message = err.to_string();
                            retry_db_action(|| db::mark_document_error(&conn, document.id, &message))
                                .map_err(|db_err| db_err.to_string())?;
                            retry_db_action(|| db::sync_document_profile(&conn, document.id))
                                .map_err(|db_err| db_err.to_string())?;
                            WorkerMessage::Failure {
                                worker_id,
                                document_id: document.id,
                                error: message,
                            }
                        }
                    };
                    if tx.send(message).is_err() {
                        break;
                    }
                }
                Ok(())
            });
            handles.push(handle);
        }

        drop(tx);

        let mut success = 0usize;
        let mut failed = 0usize;
        for message in rx {
            match message {
                WorkerMessage::Success {
                    worker_id,
                    document_id,
                    path,
                    section_count,
                    row_count,
                } => {
                    success += 1;
                    println!(
                        "[worker {}] ingested document_id={} sections={} rows={} path={}",
                        worker_id, document_id, section_count, row_count, path
                    );
                }
                WorkerMessage::Failure {
                    worker_id,
                    document_id,
                    error,
                } => {
                    failed += 1;
                    eprintln!(
                        "[worker {}] failed document_id={} error={}",
                        worker_id, document_id, error
                    );
                }
            }
        }

        for handle in handles {
            match handle.join() {
                Ok(Ok(())) => {}
                Ok(Err(err)) => return Err(err.into()),
                Err(_) => return Err("ingest worker panicked".into()),
            }
        }

        println!("ingest-documents complete");
        println!("family={}", family.unwrap_or(""));
        println!("limit={}", limit.map(|x| x.to_string()).unwrap_or_default());
        println!("jobs={worker_count}");
        println!("document_id={}", document_id.map(|value| value.to_string()).unwrap_or_default());
        println!("success={success}");
        println!("failed={failed}");
        Ok(())
    })
}

fn process_document_with_retry(
    conn: &mut rusqlite::Connection,
    document: &db::DocumentRecord,
) -> Result<(usize, usize), Box<dyn Error>> {
    const MAX_ATTEMPTS: usize = 6;
    for attempt in 1..=MAX_ATTEMPTS {
        match ingest_single_document(conn, document) {
            Ok(result) => return Ok(result),
            Err(err) if is_locked_error(&err) && attempt < MAX_ATTEMPTS => {
                let delay_ms = 150 * attempt as u64;
                eprintln!(
                    "retrying document_id={} after lock attempt={}/{} delay_ms={}",
                    document.id, attempt, MAX_ATTEMPTS, delay_ms
                );
                thread::sleep(Duration::from_millis(delay_ms));
            }
            Err(err) => return Err(err),
        }
    }

    Err("unreachable retry loop".into())
}

fn retry_db_action<T, F>(mut action: F) -> Result<T, Box<dyn Error>>
where
    F: FnMut() -> Result<T, Box<dyn Error>>,
{
    const MAX_ATTEMPTS: usize = 6;
    for attempt in 1..=MAX_ATTEMPTS {
        match action() {
            Ok(value) => return Ok(value),
            Err(err) if is_locked_error(&err) && attempt < MAX_ATTEMPTS => {
                thread::sleep(Duration::from_millis(150 * attempt as u64));
            }
            Err(err) => return Err(err),
        }
    }

    Err("unreachable retry loop".into())
}

fn is_locked_error(err: &Box<dyn Error>) -> bool {
    err.to_string().to_ascii_lowercase().contains("database is locked")
}

fn ingest_single_document(
    conn: &mut rusqlite::Connection,
    document: &db::DocumentRecord,
) -> Result<(usize, usize), Box<dyn Error>> {
    match document.extension.to_ascii_lowercase().as_str() {
        ".xlsx" | ".xlsm" => xlsx_ingest::ingest_document(conn, document)
            .map(|stats| (stats.section_count, stats.row_count)),
        ".docx" | ".pptx" | ".pptm" => ooxml_ingest::ingest_document(conn, document)
            .map(|stats| (stats.section_count, stats.row_count)),
        other => Err(format!("unsupported extension for ingest: {other}").into()),
    }
}

enum WorkerMessage {
    Success {
        worker_id: usize,
        document_id: i64,
        path: String,
        section_count: usize,
        row_count: usize,
    },
    Failure {
        worker_id: usize,
        document_id: i64,
        error: String,
    },
}

pub fn rebuild_document_index(config: &AppConfig) -> Result<(), Box<dyn Error>> {
    let conn = db::open_connection(&config.db_path)?;
    db::apply_schema(&conn)?;
    let synced = db::sync_all_document_profiles(&conn)?;
    println!("rebuild-document-index complete");
    println!("document_profiles_synced={synced}");
    Ok(())
}

pub fn search(
    config: &AppConfig,
    keyword: &str,
    family: Option<&str>,
    limit: usize,
) -> Result<(), Box<dyn Error>> {
    let conn = db::open_connection(&config.db_path)?;
    let results = db::search_rows(&conn, keyword, family, limit)?;
    println!("search keyword={keyword}");
    println!("result_count={}", results.len());
    for (idx, item) in results.iter().enumerate() {
        println!("--- result {} ---", idx + 1);
        println!("document_id={}", item.document_id);
        println!("file_name={}", item.file_name);
        println!("document_family={}", item.document_family);
        println!("original_path={}", item.original_path);
        println!("sheet_name={}", item.sheet_name);
        println!("row_index={}", item.row_index);
        println!("score={}", item.score);
        println!("equipment_no={}", item.equipment_no.as_deref().unwrap_or(""));
        println!("work_name={}", item.work_name.as_deref().unwrap_or(""));
        println!("part_name={}", item.part_name.as_deref().unwrap_or(""));
        println!("normalized_date={}", item.normalized_date.as_deref().unwrap_or(""));
        println!("row_text={}", item.row_text);
    }
    Ok(())
}

pub fn run_query(config: &AppConfig, sql: &str) -> Result<(), Box<dyn Error>> {
    let conn = db::open_connection(&config.db_path)?;
    db::run_select_query(&conn, sql)
}

pub fn evaluate(config: &AppConfig, gold_path: &str) -> Result<(), Box<dyn Error>> {
    let conn = db::open_connection(&config.db_path)?;
    let records = csv_util::read_csv_records(gold_path)?;
    let total = records.len();
    if total == 0 {
        return Err("gold file is empty".into());
    }

    let mut top1 = 0usize;
    let mut top3 = 0usize;
    let mut top5 = 0usize;
    let mut lines = vec![
        "# Evaluation Report".to_string(),
        "".to_string(),
        format!("- db: {}", config.db_path),
        format!("- gold: {}", gold_path),
        format!("- cases: {}", total),
        "".to_string(),
        "| Query | Top1 | Top3 | Top5 | Expected File |".to_string(),
        "|---|---:|---:|---:|---|".to_string(),
    ];

    for record in records {
        let query = required(&record, "query")?;
        let family = record.get("family").map(|x| x.as_str()).unwrap_or("");
        let expected_file = required(&record, "expected_file")?;
        let expected_substring = required(&record, "expected_substring")?;
        let results = db::search_rows(
            &conn,
            &query,
            if family.is_empty() { None } else { Some(family) },
            5,
        )?;

        let hit1 = hit(&results, &expected_file, &expected_substring, 1);
        let hit3 = hit(&results, &expected_file, &expected_substring, 3);
        let hit5 = hit(&results, &expected_file, &expected_substring, 5);
        top1 += usize::from(hit1);
        top3 += usize::from(hit3);
        top5 += usize::from(hit5);

        lines.push(format!(
            "| {} | {} | {} | {} | {} |",
            escape_md(&query),
            yn(hit1),
            yn(hit3),
            yn(hit5),
            escape_md(&expected_file)
        ));
    }

    let top1_acc = percent(top1, total);
    let top3_acc = percent(top3, total);
    let top5_acc = percent(top5, total);

    lines.splice(
        5..5,
        vec![
            format!("- top1_accuracy: {}% ({}/{})", top1_acc, top1, total),
            format!("- top3_accuracy: {}% ({}/{})", top3_acc, top3, total),
            format!("- top5_accuracy: {}% ({}/{})", top5_acc, top5, total),
            "".to_string(),
        ],
    );

    let report_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("eval").join("evaluation_report.md");
    fs::write(&report_path, lines.join("\n"))?;

    println!("evaluate complete");
    println!("gold={}", gold_path);
    println!("top1_accuracy={}% ({}/{})", top1_acc, top1, total);
    println!("top3_accuracy={}% ({}/{})", top3_acc, top3, total);
    println!("top5_accuracy={}% ({}/{})", top5_acc, top5, total);
    println!("report={}", report_path.display());

    Ok(())
}

fn hit(results: &[db::SearchResult], expected_file: &str, expected_substring: &str, k: usize) -> bool {
    results
        .iter()
        .take(k)
        .any(|row| row.file_name == expected_file && row.row_text.contains(expected_substring))
}

fn percent(hit: usize, total: usize) -> String {
    format!("{:.1}", (hit as f64 / total as f64) * 100.0)
}

fn yn(v: bool) -> &'static str {
    if v { "Y" } else { "N" }
}

fn escape_md(s: &str) -> String {
    s.replace('|', "\\|")
}

fn required(map: &std::collections::HashMap<String, String>, key: &str) -> Result<String, Box<dyn Error>> {
    map.get(key).cloned().ok_or_else(|| format!("missing field: {key}").into())
}
