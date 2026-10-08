use crate::{analysis, benchmark, coach, importer, model::*, storage};
use notify::{RecursiveMode, Watcher};
use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tauri::{Emitter, Manager, State};
#[derive(Clone)]
pub struct AppState {
    db: PathBuf,
    settings: Arc<Mutex<Settings>>,
    scan_lock: Arc<Mutex<()>>,
    watching: Arc<AtomicBool>,
    watch_error: Arc<Mutex<Option<String>>>,
}
#[derive(Serialize)]
pub struct Snapshot {
    settings: Settings,
    scenarios: Vec<analysis::ScenarioAnalysis>,
    today: coach::Today,
    benchmark: Option<benchmark::BenchmarkView>,
    plan: coach::Plan,
    sessions: Vec<analysis::Session>,
    reports: Vec<Report>,
    sync: ImportSummary,
    db_path: String,
    watching: bool,
    watch_error: Option<String>,
    key_configured: bool,
}
fn credential() -> Result<keyring::Entry, String> {
    keyring::Entry::new("AimCoach", "openai-compatible-api")
        .map_err(|_| "无法访问 Windows 凭据管理器".into())
}
fn snapshot_inner(s: &AppState) -> Result<Snapshot, String> {
    let c = storage::open(&s.db)?;
    let settings = storage::settings(&c)?;
    let runs = storage::runs(&c)?;
    let b = storage::benchmark(&c)?;
    let now = chrono::Utc::now().timestamp();
    let bv = b
        .as_ref()
        .map(|b| benchmark::view(b, &runs, &settings.target_rank, now));
    let plan = coach::plan(bv.as_ref(), &settings, &runs, now);
    let mut sessions = analysis::sessions(&runs, settings.session_gap_minutes);
    sessions.reverse();
    sessions.truncate(20);
    Ok(Snapshot {
        settings: settings.clone(),
        scenarios: analysis::all(&runs, now),
        today: coach::today(&runs, b.as_ref(), now),
        benchmark: bv,
        plan,
        sessions,
        reports: storage::reports(&c)?,
        sync: storage::sync(&c)?,
        db_path: s.db.to_string_lossy().into(),
        watching: s.watching.load(Ordering::Relaxed),
        watch_error: s.watch_error.lock().map_err(|_| "监听状态锁错误")?.clone(),
        key_configured: credential()
            .and_then(|e| e.get_password().map_err(|_| String::new()))
            .is_ok(),
    })
}
fn scan_inner(s: &AppState, force: bool) -> Result<ImportSummary, String> {
    let _guard = s.scan_lock.lock().map_err(|_| "扫描任务锁错误")?;
    let dir = s
        .settings
        .lock()
        .map_err(|_| "设置锁错误")?
        .stats_dir
        .clone();
    let result = importer::scan(&s.db, Path::new(&dir), force)?;
    if result.imported + result.updated > 0 {
        let c = storage::open(&s.db)?;
        let settings = storage::settings(&c)?;
        let rs = storage::runs(&c)?;
        let b = storage::benchmark(&c)?;
        let mut report =
            coach::local_report(&rs, b.as_ref(), &settings, chrono::Utc::now().timestamp());
        storage::save_report(&c, &mut report)?;
    }
    Ok(result)
}
#[tauri::command]
async fn snapshot(state: State<'_, AppState>) -> Result<Snapshot, String> {
    let s = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || snapshot_inner(&s))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn scan_stats(state: State<'_, AppState>, force: bool) -> Result<ImportSummary, String> {
    let s = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || scan_inner(&s, force))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn save_settings(state: State<'_, AppState>, settings: Settings) -> Result<(), String> {
    settings.validate()?;
    if !settings.stats_dir.is_empty() && !Path::new(&settings.stats_dir).is_dir() {
        return Err("Stats 目录不存在".into());
    }
    let s = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = s.scan_lock.lock().map_err(|_| "扫描任务锁错误")?;
        let mut c = storage::open(&s.db)?;
        storage::save_settings(&c, &settings)?;
        storage::regroup(&mut c, settings.session_gap_minutes)?;
        *s.settings.lock().map_err(|_| "设置锁错误")? = settings;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn history(
    state: State<'_, AppState>,
    name: String,
    from: Option<i64>,
    to: Option<i64>,
) -> Result<serde_json::Value, String> {
    let s = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let c = storage::open(&s.db)?;
        let rs: Vec<_> = storage::runs(&c)?
            .into_iter()
            .filter(|r| {
                r.scenario == name
                    && from.is_none_or(|v| r.timestamp >= v)
                    && to.is_none_or(|v| r.timestamp <= v)
            })
            .map(|mut r| {
                r.raw = serde_json::Value::Null;
                r
            })
            .collect();
        let mut high = 0.0_f64;
        let pb: Vec<_> = rs
            .iter()
            .map(|r| {
                high = high.max(r.score);
                high
            })
            .collect();
        Ok(serde_json::json!({"runs":rs,"pb_curve":pb}))
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn scenario_analysis(
    state: State<'_, AppState>,
    name: String,
    from: Option<i64>,
    to: Option<i64>,
) -> Result<analysis::ScenarioAnalysis, String> {
    let s = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let c = storage::open(&s.db)?;
        let runs: Vec<_> = storage::runs(&c)?
            .into_iter()
            .filter(|r| {
                from.is_none_or(|v| r.timestamp >= v) && to.is_none_or(|v| r.timestamp <= v)
            })
            .collect();
        Ok(analysis::scenario(
            &name,
            &runs,
            chrono::Utc::now().timestamp(),
        ))
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
fn detect_stats() -> Vec<String> {
    importer::detect_stats()
}
#[tauri::command]
async fn import_benchmark(state: State<'_, AppState>, json: String) -> Result<(), String> {
    if json.len() > 2 * 1024 * 1024 {
        return Err("JSON 超过 2 MB".into());
    }
    let b: BenchmarkConfig =
        serde_json::from_str(&json).map_err(|e| format!("JSON 格式错误：{e}"))?;
    benchmark::validate(&b)?;
    let s = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut c = storage::open(&s.db)?;
        storage::save_benchmark(&mut c, &b)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn read_benchmark_file(path: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let p = Path::new(&path);
        if p.extension()
            .is_none_or(|x| !x.eq_ignore_ascii_case("json"))
        {
            return Err("请选择 JSON 文件".into());
        }
        if std::fs::metadata(p).map_err(|e| e.to_string())?.len() > 2 * 1024 * 1024 {
            return Err("JSON 超过 2 MB".into());
        }
        std::fs::read_to_string(p).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
fn save_api_key(key: String) -> Result<(), String> {
    if key.is_empty() {
        return Err("API Key 不能为空".into());
    }
    credential()?
        .set_password(&key)
        .map_err(|_| "Windows 凭据保存失败；API Key 未保存到数据库".into())
}
#[tauri::command]
fn delete_api_key() -> Result<(), String> {
    match credential()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err("删除凭据失败".into()),
    }
}
#[tauri::command]
async fn generate_report(state: State<'_, AppState>, use_ai: bool) -> Result<Report, String> {
    let s = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let c = storage::open(&s.db)?;
        let settings = storage::settings(&c)?;
        let rs = storage::runs(&c)?;
        let b = storage::benchmark(&c)?;
        let local = coach::local_report(&rs, b.as_ref(), &settings, chrono::Utc::now().timestamp());
        let mut r = if use_ai {
            if !settings.ai_consent {
                return Err("请先在设置中明确授权聚合数据离开本机".into());
            }
            coach::explain_with(local, |ev| {
                let key = credential()?
                    .get_password()
                    .map_err(|_| "未配置 API Key".to_string())?;
                coach::api_explain(ev, &settings, &key)
            })
        } else {
            local
        };
        storage::save_report(&c, &mut r)?;
        Ok(r)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn backup_database(state: State<'_, AppState>, target: String) -> Result<(), String> {
    let s = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let p = Path::new(&target);
        if p.exists() {
            return Err("备份文件已存在，请选择新文件名".into());
        }
        if p.extension()
            .is_none_or(|x| !x.eq_ignore_ascii_case("sqlite"))
        {
            return Err("备份扩展名必须为 .sqlite".into());
        }
        let settings = s.settings.lock().map_err(|_| "设置锁错误")?.clone();
        let parent = p
            .parent()
            .ok_or("无效路径")?
            .canonicalize()
            .map_err(|e| e.to_string())?;
        if !settings.stats_dir.is_empty() {
            let stats = Path::new(&settings.stats_dir)
                .canonicalize()
                .map_err(|e| e.to_string())?;
            if parent.starts_with(stats) {
                return Err("为保护原始数据，请将备份保存在 Stats 之外".into());
            }
        }
        let c = storage::open(&s.db)?;
        c.backup("main", p, None).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}
fn watcher(app: tauri::AppHandle, state: AppState) {
    std::thread::spawn(move || {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut active: Option<notify::RecommendedWatcher> = None;
        let mut last = String::new();
        let mut enabled = false;
        let mut pending = false;
        let mut deadline = std::time::Instant::now();
        loop {
            let settings = match state.settings.lock() {
                Ok(s) => s.clone(),
                Err(_) => return,
            };
            if last != settings.stats_dir || enabled != settings.auto_watch {
                active = None;
                state.watching.store(false, Ordering::Relaxed);
                *state.watch_error.lock().unwrap() = None;
                last = settings.stats_dir.clone();
                enabled = settings.auto_watch;
                if enabled && !last.is_empty() {
                    let tx2 = tx.clone();
                    match notify::recommended_watcher(
                        move |event: notify::Result<notify::Event>| {
                            let _ = tx2.send(event);
                        },
                    ) {
                        Ok(mut w) => match w.watch(Path::new(&last), RecursiveMode::Recursive) {
                            Ok(()) => {
                                active = Some(w);
                                state.watching.store(true, Ordering::Relaxed);
                                pending = true;
                                deadline = std::time::Instant::now() + Duration::from_secs(2);
                            }
                            Err(e) => *state.watch_error.lock().unwrap() = Some(e.to_string()),
                        },
                        Err(e) => *state.watch_error.lock().unwrap() = Some(e.to_string()),
                    }
                }
                // Refresh the UI after changing the watched path or toggle.
                let _ = app.emit("stats-synced", ());
            }
            match rx.recv_timeout(Duration::from_secs(1)) {
                Ok(Ok(e))
                    if !matches!(e.kind, notify::EventKind::Access(_))
                        && e.paths.iter().any(|p| {
                            p.extension().is_some_and(|x| x.eq_ignore_ascii_case("csv"))
                        }) =>
                {
                    pending = true;
                    deadline = std::time::Instant::now() + Duration::from_secs(2);
                }
                Ok(Err(e)) => {
                    *state.watch_error.lock().unwrap() = Some(e.to_string());
                }
                _ => {}
            }
            if active.is_some() && pending && std::time::Instant::now() >= deadline {
                match scan_inner(&state, false) {
                    Ok(summary) => {
                        pending = summary.deferred > 0;
                        deadline = std::time::Instant::now() + Duration::from_secs(3);
                        let _ = app.emit("stats-synced", &summary);
                    }
                    Err(e) => {
                        pending = false;
                        *state.watch_error.lock().unwrap() = Some(e);
                        let _ = app.emit("stats-synced", ());
                    }
                }
            }
        }
    });
}
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let dir = std::env::var_os("AIMCOACH_DATA_DIR")
                .map(PathBuf::from)
                .unwrap_or(app.path().app_data_dir()?);
            std::fs::create_dir_all(&dir)?;
            let db = dir.join("aimcoach.sqlite");
            let c = storage::open(&db).map_err(std::io::Error::other)?;
            let mut settings = storage::settings(&c).map_err(std::io::Error::other)?;
            if settings.stats_dir.is_empty() {
                if let Some(path) = importer::detect_stats().first() {
                    settings.stats_dir = path.clone();
                    storage::save_settings(&c, &settings).map_err(std::io::Error::other)?;
                }
            }
            let state = AppState {
                db,
                settings: Arc::new(Mutex::new(settings)),
                scan_lock: Arc::new(Mutex::new(())),
                watching: Arc::new(AtomicBool::new(false)),
                watch_error: Arc::new(Mutex::new(None)),
            };
            app.manage(state.clone());
            watcher(app.handle().clone(), state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            snapshot,
            scan_stats,
            save_settings,
            history,
            scenario_analysis,
            detect_stats,
            import_benchmark,
            read_benchmark_file,
            save_api_key,
            delete_api_key,
            generate_report,
            backup_database
        ])
        .run(tauri::generate_context!())
        .expect("无法启动 AimCoach");
}
