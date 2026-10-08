use crate::model::*;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
pub type Result<T> = std::result::Result<T, String>;
pub fn open(path: &Path) -> Result<Connection> {
    let c = Connection::open(path).map_err(|e| e.to_string())?;
    c.busy_timeout(std::time::Duration::from_secs(15))
        .map_err(|e| e.to_string())?;
    c.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL;")
        .map_err(|e| e.to_string())?;
    let version: i64 = c
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    if version == 0 {
        c.execute_batch(include_str!("../migrations/001_initial.sql"))
            .map_err(|e| e.to_string())?;
    }
    if version > 1 {
        return Err("数据库版本高于此应用；请使用新版 AimCoach".into());
    }
    Ok(c)
}
pub fn settings(c: &Connection) -> Result<Settings> {
    let json: Option<String> = c
        .query_row("SELECT json FROM settings WHERE id=1", [], |r| r.get(0))
        .optional()
        .map_err(|e| e.to_string())?;
    json.map(|j| serde_json::from_str(&j).map_err(|e| e.to_string()))
        .unwrap_or(Ok(Settings::default()))
}
pub fn save_settings(c: &Connection, s: &Settings) -> Result<()> {
    s.validate()?;
    c.execute(
        "INSERT INTO settings VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET json=excluded.json",
        [serde_json::to_string(s).unwrap()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
pub fn runs(c: &Connection) -> Result<Vec<TrainingRun>> {
    let mut st=c.prepare("SELECT r.run_id,s.name,r.timestamp,r.timestamp_basis,r.score,r.accuracy,r.kills,r.shots,r.hits,r.duration,r.source_file,r.raw_json FROM training_run r JOIN scenario s USING(scenario_id) ORDER BY r.timestamp,r.run_id").map_err(|e|e.to_string())?;
    let rows = st
        .query_map([], |r| {
            Ok(TrainingRun {
                run_id: r.get(0)?,
                scenario: r.get(1)?,
                timestamp: r.get(2)?,
                timestamp_basis: r.get(3)?,
                score: r.get(4)?,
                accuracy: r.get(5)?,
                kills: r.get(6)?,
                shots: r.get(7)?,
                hits: r.get(8)?,
                duration: r.get(9)?,
                source_file: r.get(10)?,
                raw: serde_json::from_str(&r.get::<_, String>(11)?)
                    .unwrap_or(serde_json::Value::Null),
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<std::result::Result<_, _>>()
        .map_err(|e| e.to_string())
}
pub fn regroup(c: &mut Connection, gap: u32) -> Result<()> {
    let rs = runs(c)?;
    let tx = c.transaction().map_err(|e| e.to_string())?;
    tx.execute("UPDATE training_run SET session_id=NULL", [])
        .map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM training_session", [])
        .map_err(|e| e.to_string())?;
    let mut sid = 0;
    let mut last = None;
    for r in rs {
        if last.is_none_or(|t| r.timestamp - t > gap as i64 * 60) {
            sid += 1;
            tx.execute(
                "INSERT INTO training_session VALUES(?1,?2,?2,0)",
                params![sid, r.timestamp],
            )
            .map_err(|e| e.to_string())?;
        }
        tx.execute(
            "UPDATE training_session SET ended_at=?2,run_count=run_count+1 WHERE session_id=?1",
            params![sid, r.timestamp],
        )
        .map_err(|e| e.to_string())?;
        tx.execute(
            "UPDATE training_run SET session_id=?2 WHERE run_id=?1",
            params![r.run_id, sid],
        )
        .map_err(|e| e.to_string())?;
        last = Some(r.timestamp);
    }
    tx.commit().map_err(|e| e.to_string())
}
pub fn benchmark(c: &Connection) -> Result<Option<BenchmarkConfig>> {
    let j: Option<String> = c
        .query_row(
            "SELECT config_json FROM benchmark WHERE benchmark_id=1",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    j.map(|j| serde_json::from_str(&j).map_err(|e| e.to_string()))
        .transpose()
}
pub fn save_benchmark(c: &mut Connection, b: &BenchmarkConfig) -> Result<()> {
    crate::benchmark::validate(b)?;
    let tx = c.transaction().map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM benchmark", [])
        .map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM scenario_skill_tag", [])
        .map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO benchmark VALUES(1,?1,?2,?3,?4)",
        params![
            b.name,
            b.version,
            b.difficulty,
            serde_json::to_string(b).unwrap()
        ],
    )
    .map_err(|e| e.to_string())?;
    for s in &b.scenarios {
        for (rank, score) in &s.thresholds {
            tx.execute(
                "INSERT INTO benchmark_threshold VALUES(1,?1,?2,?3)",
                params![s.name, rank, score],
            )
            .map_err(|e| e.to_string())?;
        }
        for tag in s.tags.iter().chain(std::iter::once(&s.subcategory)) {
            tx.execute(
                "INSERT OR IGNORE INTO scenario_skill_tag VALUES(?1,?2,?3,?4)",
                params![s.name, s.category, s.subcategory, tag],
            )
            .map_err(|e| e.to_string())?;
        }
    }
    tx.commit().map_err(|e| e.to_string())
}
pub fn save_sync(c: &Connection, s: &ImportSummary) -> Result<()> {
    c.execute(
        "INSERT INTO sync_state VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET json=excluded.json",
        [serde_json::to_string(s).unwrap()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
pub fn sync(c: &Connection) -> Result<ImportSummary> {
    let j: Option<String> = c
        .query_row("SELECT json FROM sync_state", [], |r| r.get(0))
        .optional()
        .map_err(|e| e.to_string())?;
    Ok(j.and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default())
}
pub fn save_report(c: &Connection, r: &mut Report) -> Result<()> {
    c.execute("INSERT INTO coach_report(generated_at,date,text,kind,evidence_json) VALUES(?1,?2,?3,?4,?5)",params![r.generated_at,r.date,r.text,r.kind,r.evidence.to_string()]).map_err(|e|e.to_string())?;
    r.id = c.last_insert_rowid();
    Ok(())
}
pub fn reports(c: &Connection) -> Result<Vec<Report>> {
    let mut st=c.prepare("SELECT report_id,generated_at,date,text,kind,evidence_json FROM coach_report ORDER BY generated_at DESC LIMIT 100").map_err(|e|e.to_string())?;
    let rs = st
        .query_map([], |r| {
            Ok(Report {
                id: r.get(0)?,
                generated_at: r.get(1)?,
                date: r.get(2)?,
                text: r.get(3)?,
                kind: r.get(4)?,
                evidence: serde_json::from_str(&r.get::<_, String>(5)?).unwrap_or_default(),
            })
        })
        .map_err(|e| e.to_string())?;
    rs.collect::<std::result::Result<_, _>>()
        .map_err(|e| e.to_string())
}
