PRAGMA foreign_keys = ON;
CREATE TABLE IF NOT EXISTS settings (id INTEGER PRIMARY KEY CHECK(id=1), json TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS scenario (scenario_id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE);
CREATE TABLE IF NOT EXISTS training_session (session_id INTEGER PRIMARY KEY, started_at INTEGER NOT NULL, ended_at INTEGER NOT NULL, run_count INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS training_run (
 run_id TEXT PRIMARY KEY, scenario_id INTEGER NOT NULL REFERENCES scenario(scenario_id),
 timestamp INTEGER NOT NULL, timestamp_basis TEXT NOT NULL, score REAL NOT NULL,
 accuracy REAL, kills REAL, shots REAL, hits REAL, duration REAL,
 source_file TEXT NOT NULL, import_timestamp INTEGER NOT NULL, raw_json TEXT NOT NULL,
 session_id INTEGER REFERENCES training_session(session_id)
);
CREATE INDEX IF NOT EXISTS idx_run_scenario_time ON training_run(scenario_id,timestamp);
CREATE INDEX IF NOT EXISTS idx_run_time ON training_run(timestamp);
CREATE TABLE IF NOT EXISTS imported_file (path TEXT PRIMARY KEY, content_hash TEXT NOT NULL, imported_at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS benchmark (benchmark_id INTEGER PRIMARY KEY CHECK(benchmark_id=1), name TEXT NOT NULL, version TEXT NOT NULL, difficulty TEXT NOT NULL, config_json TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS benchmark_threshold (benchmark_id INTEGER NOT NULL REFERENCES benchmark(benchmark_id) ON DELETE CASCADE, scenario_name TEXT NOT NULL, rank_name TEXT NOT NULL, score REAL NOT NULL, PRIMARY KEY(benchmark_id,scenario_name,rank_name));
CREATE TABLE IF NOT EXISTS scenario_skill_tag (scenario_name TEXT NOT NULL, category TEXT NOT NULL, subcategory TEXT NOT NULL, tag TEXT NOT NULL, PRIMARY KEY(scenario_name,tag));
CREATE TABLE IF NOT EXISTS coach_report (report_id INTEGER PRIMARY KEY, generated_at INTEGER NOT NULL, date TEXT NOT NULL, text TEXT NOT NULL, kind TEXT NOT NULL, evidence_json TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sync_state (id INTEGER PRIMARY KEY CHECK(id=1), json TEXT NOT NULL);
PRAGMA user_version=1;
