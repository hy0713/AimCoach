use crate::{
    model::*,
    parser::{KovaakCsvParser, RunParser},
    storage,
};
use rusqlite::{params, OptionalExtension};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

fn files(dir: &Path, result: &mut Vec<PathBuf>, errors: &mut Vec<String>) {
    match fs::read_dir(dir) {
        Ok(entries) => {
            for entry in entries {
                match entry {
                    Ok(e) => match e.file_type() {
                        Ok(t) if t.is_dir() => files(&e.path(), result, errors),
                        Ok(t)
                            if t.is_file()
                                && e.path()
                                    .extension()
                                    .is_some_and(|s| s.eq_ignore_ascii_case("csv")) =>
                        {
                            result.push(e.path())
                        }
                        Err(e) => errors.push(e.to_string()),
                        _ => {}
                    },
                    Err(e) => errors.push(e.to_string()),
                }
            }
        }
        Err(e) => errors.push(format!("{}：{e}", dir.display())),
    }
}
/// Files newer than 2s are deferred; metadata and a second read must agree.
pub fn stable_read(path: &Path) -> Result<Option<Vec<u8>>, String> {
    let a = fs::metadata(path).map_err(|e| e.to_string())?;
    if a.len() > 32 * 1024 * 1024 {
        return Err("CSV 超过 32 MB，已拒绝以防 UI/内存耗尽".into());
    }
    if a.modified()
        .ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .is_some_and(|d| d < Duration::from_secs(2))
    {
        return Ok(None);
    }
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    std::thread::sleep(Duration::from_millis(80));
    let b = fs::metadata(path).map_err(|e| e.to_string())?;
    if a.len() != b.len() || a.modified().ok() != b.modified().ok() {
        return Ok(None);
    }
    if fs::read(path).map_err(|e| e.to_string())? != bytes {
        return Ok(None);
    }
    Ok(Some(bytes))
}
pub fn import_bytes(
    c: &mut rusqlite::Connection,
    path: &Path,
    bytes: &[u8],
    force: bool,
) -> Result<bool, String> {
    let hash = format!("{:x}", Sha256::digest(bytes));
    let name = path.to_string_lossy().to_string();
    let old: Option<String> = c
        .query_row(
            "SELECT content_hash FROM imported_file WHERE path=?1",
            [&name],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if !force && old.as_ref() == Some(&hash) {
        return Ok(false);
    }
    let r = KovaakCsvParser.parse(path, bytes)?;
    let tx = c.transaction().map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT OR IGNORE INTO scenario(name) VALUES(?1)",
        [&r.scenario],
    )
    .map_err(|e| e.to_string())?;
    let sid: i64 = tx
        .query_row(
            "SELECT scenario_id FROM scenario WHERE name=?1",
            [&r.scenario],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().timestamp();
    let exists: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM training_run WHERE run_id=?1)",
            [&r.run_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    tx.execute("INSERT INTO training_run(run_id,scenario_id,timestamp,timestamp_basis,score,accuracy,kills,shots,hits,duration,source_file,import_timestamp,raw_json) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13) ON CONFLICT(run_id) DO UPDATE SET score=excluded.score,accuracy=excluded.accuracy,kills=excluded.kills,shots=excluded.shots,hits=excluded.hits,duration=excluded.duration,raw_json=excluded.raw_json,timestamp_basis=excluded.timestamp_basis,import_timestamp=excluded.import_timestamp",params![r.run_id,sid,r.timestamp,r.timestamp_basis,r.score,r.accuracy,r.kills,r.shots,r.hits,r.duration,r.source_file,now,r.raw.to_string()]).map_err(|e|e.to_string())?;
    tx.execute("INSERT INTO imported_file VALUES(?1,?2,?3) ON CONFLICT(path) DO UPDATE SET content_hash=excluded.content_hash,imported_at=excluded.imported_at",params![name,hash,now]).map_err(|e|e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(!exists)
}
pub fn scan(db: &Path, dir: &Path, force: bool) -> Result<ImportSummary, String> {
    if !dir.is_dir() {
        return Err(format!("Stats 目录不存在：{}", dir.display()));
    }
    let mut c = storage::open(db)?;
    let mut summary = ImportSummary::default();
    let mut paths = Vec::new();
    files(dir, &mut paths, &mut summary.errors);
    paths.sort();
    summary.failed = summary.errors.len();
    for p in paths {
        // Hash unchanged files before the stabilization delay. A changed file
        // still goes through both reads and metadata checks before parsing.
        if !force {
            let old: Option<String> = c
                .query_row(
                    "SELECT content_hash FROM imported_file WHERE path=?1",
                    [p.to_string_lossy().as_ref()],
                    |r| r.get(0),
                )
                .optional()
                .map_err(|e| e.to_string())?;
            if let Some(old) = old {
                if fs::metadata(&p).is_ok_and(|m| m.len() <= 32 * 1024 * 1024) {
                    if let Ok(bytes) = fs::read(&p) {
                        if format!("{:x}", Sha256::digest(&bytes)) == old {
                            summary.skipped += 1;
                            continue;
                        }
                    }
                }
            }
        }
        match stable_read(&p) {
            Ok(None) => summary.deferred += 1,
            Ok(Some(b)) => match import_bytes(&mut c, &p, &b, force) {
                Ok(true) => summary.imported += 1,
                Ok(false) => summary.updated += 1,
                Err(e) => {
                    summary.failed += 1;
                    if summary.errors.len() < 100 {
                        summary.errors.push(format!("{}：{e}", p.display()));
                    }
                }
            },
            Err(e) => {
                summary.failed += 1;
                if summary.errors.len() < 100 {
                    summary.errors.push(format!("{}：{e}", p.display()));
                }
            }
        }
    }
    let gap = storage::settings(&c)?.session_gap_minutes;
    storage::regroup(&mut c, gap)?;
    summary.finished_at = Some(chrono::Utc::now().timestamp());
    storage::save_sync(&c, &summary)?;
    Ok(summary)
}
pub fn detect_stats() -> Vec<String> {
    let mut roots = Vec::new();
    for base in [
        std::env::var("ProgramFiles(x86)").unwrap_or_default(),
        std::env::var("ProgramFiles").unwrap_or_default(),
    ] {
        roots.push(PathBuf::from(base).join("Steam"));
    }
    // Steam registry helps find nondefault Steam installations.
    #[cfg(windows)]
    {
        if let Ok(o) = std::process::Command::new("reg")
            .args(["query", r"HKCU\Software\Valve\Steam", "/v", "SteamPath"])
            .output()
        {
            let s = String::from_utf8_lossy(&o.stdout);
            for line in s.lines() {
                if let Some((_, p)) = line.split_once("REG_SZ") {
                    roots.push(PathBuf::from(p.trim()));
                }
            }
        }
    }
    for ch in 'C'..='Z' {
        roots.push(PathBuf::from(format!("{ch}:\\SteamLibrary")));
        roots.push(PathBuf::from(format!("{ch}:\\Steam")));
    }
    let re = regex::Regex::new(r#""path"\s+"([^"]+)""#).unwrap();
    for root in roots.clone() {
        if let Ok(v) = fs::read_to_string(root.join("steamapps/libraryfolders.vdf")) {
            for cap in re.captures_iter(&v) {
                roots.push(PathBuf::from(cap[1].replace("\\\\", "\\")));
            }
        }
    }
    let mut result: Vec<String> = roots
        .iter()
        .map(|r| r.join("steamapps/common/FPSAimTrainer/FPSAimTrainer/stats"))
        .filter(|p| p.is_dir())
        .map(|p| p.to_string_lossy().into())
        .collect();
    result.sort();
    result.dedup();
    result
}
