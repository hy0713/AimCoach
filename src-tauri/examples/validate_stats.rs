//! Optional read-only validation against a user-selected real Stats folder.
use aimcoach_core::{analysis, coach, importer, model::Settings, storage};
use std::path::PathBuf;
fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: validate_stats <Stats folder> <isolated validation.sqlite>");
        std::process::exit(2);
    }
    let folder = PathBuf::from(&args[1]);
    let db = PathBuf::from(&args[2]);
    let first = importer::scan(&db, &folder, false).expect("first scan");
    let second = importer::scan(&db, &folder, false).expect("second scan");
    let c = storage::open(&db).unwrap();
    let rs = storage::runs(&c).unwrap();
    let now = chrono::Utc::now().timestamp();
    let analyses = analysis::all(&rs, now);
    let report = coach::local_report(&rs, None, &Settings::default(), now);
    let output = serde_json::json!({"first_scan":first,"repeat_scan":second,"stored_runs":rs.len(),"scenarios":analyses.len(),"chronological":rs.windows(2).all(|p|p[0].timestamp<=p[1].timestamp),"accuracy_known":rs.iter().filter(|r|r.accuracy.is_some()).count(),"duration_known":rs.iter().filter(|r|r.duration.is_some()).count(),"rule_report_generated":!report.text.is_empty(),"stable_improvement_scenarios":analyses.iter().filter(|a|a.verdict=="improved").count()});
    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}
