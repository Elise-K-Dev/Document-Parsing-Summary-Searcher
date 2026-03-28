-- Evidence search database schema
-- Primary target: SQLite
-- Secondary target: PostgreSQL with minor type/index adjustments

PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS documents (
    id INTEGER PRIMARY KEY,
    original_path TEXT NOT NULL UNIQUE,
    staged_path TEXT NOT NULL,
    file_name TEXT NOT NULL,
    extension TEXT NOT NULL,
    document_family TEXT NOT NULL,
    sha256 TEXT NOT NULL,
    size_bytes INTEGER NOT NULL DEFAULT 0,
    last_write_time TEXT NOT NULL,
    parse_status TEXT NOT NULL DEFAULT 'pending',
    parse_error TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_documents_family ON documents(document_family);
CREATE INDEX IF NOT EXISTS idx_documents_sha256 ON documents(sha256);
CREATE INDEX IF NOT EXISTS idx_documents_status ON documents(parse_status);

CREATE TABLE IF NOT EXISTS sections (
    id INTEGER PRIMARY KEY,
    document_id INTEGER NOT NULL,
    section_type TEXT NOT NULL,
    section_order INTEGER NOT NULL DEFAULT 0,
    sheet_name TEXT,
    page_no INTEGER,
    title TEXT,
    raw_text TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (document_id) REFERENCES documents(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_sections_document_id ON sections(document_id);
CREATE INDEX IF NOT EXISTS idx_sections_sheet_name ON sections(sheet_name);

CREATE TABLE IF NOT EXISTS tables_meta (
    id INTEGER PRIMARY KEY,
    document_id INTEGER NOT NULL,
    section_id INTEGER,
    table_name TEXT,
    table_order INTEGER NOT NULL DEFAULT 0,
    header_json TEXT NOT NULL,
    header_text TEXT NOT NULL,
    row_count INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (document_id) REFERENCES documents(id) ON DELETE CASCADE,
    FOREIGN KEY (section_id) REFERENCES sections(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_tables_document_id ON tables_meta(document_id);
CREATE INDEX IF NOT EXISTS idx_tables_section_id ON tables_meta(section_id);

CREATE TABLE IF NOT EXISTS table_rows (
    id INTEGER PRIMARY KEY,
    document_id INTEGER NOT NULL,
    section_id INTEGER,
    table_id INTEGER,
    row_index INTEGER NOT NULL,
    row_text TEXT NOT NULL,
    row_json TEXT NOT NULL,
    normalized_date TEXT,
    equipment_no TEXT,
    work_name TEXT,
    part_name TEXT,
    assignee TEXT,
    work_type TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (document_id) REFERENCES documents(id) ON DELETE CASCADE,
    FOREIGN KEY (section_id) REFERENCES sections(id) ON DELETE SET NULL,
    FOREIGN KEY (table_id) REFERENCES tables_meta(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_rows_document_id ON table_rows(document_id);
CREATE INDEX IF NOT EXISTS idx_rows_table_id ON table_rows(table_id);
CREATE INDEX IF NOT EXISTS idx_rows_date ON table_rows(normalized_date);
CREATE INDEX IF NOT EXISTS idx_rows_equipment ON table_rows(equipment_no);
CREATE INDEX IF NOT EXISTS idx_rows_work_name ON table_rows(work_name);
CREATE INDEX IF NOT EXISTS idx_rows_part_name ON table_rows(part_name);
CREATE INDEX IF NOT EXISTS idx_rows_assignee ON table_rows(assignee);

CREATE TABLE IF NOT EXISTS cells (
    id INTEGER PRIMARY KEY,
    row_id INTEGER NOT NULL,
    column_index INTEGER NOT NULL,
    column_name TEXT,
    cell_value TEXT,
    normalized_value TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (row_id) REFERENCES table_rows(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_cells_row_id ON cells(row_id);
CREATE INDEX IF NOT EXISTS idx_cells_column_name ON cells(column_name);

CREATE TABLE IF NOT EXISTS entities (
    id INTEGER PRIMARY KEY,
    document_id INTEGER NOT NULL,
    row_id INTEGER,
    entity_type TEXT NOT NULL,
    entity_value TEXT NOT NULL,
    normalized_value TEXT NOT NULL,
    confidence REAL NOT NULL DEFAULT 1.0,
    source_column TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (document_id) REFERENCES documents(id) ON DELETE CASCADE,
    FOREIGN KEY (row_id) REFERENCES table_rows(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_entities_document_id ON entities(document_id);
CREATE INDEX IF NOT EXISTS idx_entities_row_id ON entities(row_id);
CREATE INDEX IF NOT EXISTS idx_entities_type_value ON entities(entity_type, normalized_value);

CREATE TABLE IF NOT EXISTS dictionaries (
    id INTEGER PRIMARY KEY,
    dict_type TEXT NOT NULL,
    source_term TEXT NOT NULL,
    normalized_term TEXT NOT NULL,
    synonym_group TEXT,
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (dict_type, source_term)
);

CREATE INDEX IF NOT EXISTS idx_dictionaries_type ON dictionaries(dict_type);
CREATE INDEX IF NOT EXISTS idx_dictionaries_norm_term ON dictionaries(normalized_term);

CREATE TABLE IF NOT EXISTS related_matches (
    id INTEGER PRIMARY KEY,
    source_document_id INTEGER NOT NULL,
    source_row_id INTEGER,
    target_document_id INTEGER NOT NULL,
    target_row_id INTEGER,
    total_score REAL NOT NULL,
    equipment_score REAL NOT NULL DEFAULT 0,
    work_score REAL NOT NULL DEFAULT 0,
    part_score REAL NOT NULL DEFAULT 0,
    date_score REAL NOT NULL DEFAULT 0,
    assignee_score REAL NOT NULL DEFAULT 0,
    family_score REAL NOT NULL DEFAULT 0,
    match_reason_json TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (source_document_id) REFERENCES documents(id) ON DELETE CASCADE,
    FOREIGN KEY (source_row_id) REFERENCES table_rows(id) ON DELETE CASCADE,
    FOREIGN KEY (target_document_id) REFERENCES documents(id) ON DELETE CASCADE,
    FOREIGN KEY (target_row_id) REFERENCES table_rows(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_related_source_doc ON related_matches(source_document_id);
CREATE INDEX IF NOT EXISTS idx_related_source_row ON related_matches(source_row_id);
CREATE INDEX IF NOT EXISTS idx_related_target_doc ON related_matches(target_document_id);
CREATE INDEX IF NOT EXISTS idx_related_score ON related_matches(total_score DESC);

CREATE TABLE IF NOT EXISTS ingest_jobs (
    id INTEGER PRIMARY KEY,
    job_type TEXT NOT NULL,
    input_path TEXT NOT NULL,
    started_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    finished_at TEXT,
    status TEXT NOT NULL DEFAULT 'running',
    processed_count INTEGER NOT NULL DEFAULT 0,
    success_count INTEGER NOT NULL DEFAULT 0,
    error_count INTEGER NOT NULL DEFAULT 0,
    note TEXT
);

CREATE TABLE IF NOT EXISTS ingest_job_items (
    id INTEGER PRIMARY KEY,
    job_id INTEGER NOT NULL,
    document_path TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    message TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (job_id) REFERENCES ingest_jobs(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_job_items_job_id ON ingest_job_items(job_id);
CREATE INDEX IF NOT EXISTS idx_job_items_status ON ingest_job_items(status);

CREATE TABLE IF NOT EXISTS document_profiles (
    document_id INTEGER PRIMARY KEY,
    section_count INTEGER NOT NULL DEFAULT 0,
    sheet_count INTEGER NOT NULL DEFAULT 0,
    row_count INTEGER NOT NULL DEFAULT 0,
    preview_text TEXT NOT NULL DEFAULT '',
    content_text TEXT NOT NULL DEFAULT '',
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (document_id) REFERENCES documents(id) ON DELETE CASCADE
);

-- SQLite FTS5 index for row-level evidence search.
CREATE VIRTUAL TABLE IF NOT EXISTS row_search USING fts5(
    row_id UNINDEXED,
    document_id UNINDEXED,
    document_family,
    sheet_name,
    header_text,
    row_text,
    equipment_no,
    work_name,
    part_name,
    assignee,
    tokenize = 'unicode61'
);
