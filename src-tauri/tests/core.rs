use aimcoach_core::{
    analysis, benchmark, coach, importer,
    model::*,
    parser::{KovaakCsvParser, RunParser},
    storage,
};
use std::{
    fs,
    path::Path,
    time::{Duration, SystemTime},
};

#[test]
fn api_url_requires_actual_local_host_for_http() {
    let mut s = Settings::default();
    for invalid in [
        "http://example.com/v1",
        "http://127.0.0.1:8000@evil.example/v1",
        "https://example.com/v1?key=secret",
        "not a URL",
    ] {
        s.ai_base_url = invalid.into();
        assert!(s.validate().is_err());
    }
    for valid in [
        "https://example.com/v1",
        "http://127.0.0.1:8000/v1",
        "http://localhost:8000/v1",
    ] {
        s.ai_base_url = valid.into();
        assert!(s.validate().is_ok());
    }
}

#[test]
fn ai_invented_percentage_is_rejected_and_rounded_evidence_is_allowed() {
    let evidence =
        serde_json::json!({"scenarios":[{"median_change":5.2345,"pb_change":-4.3}],"raw_score":99});
    assert!(coach::valid_ai_percentages(
        "中位数提升5.2%，PB变化4.3%",
        &evidence
    ));
    assert!(!coach::valid_ai_percentages("提升99%", &evidence));
    let mut r = coach::local_report(&[], None, &Settings::default(), 0);
    r.evidence = evidence;
    assert_eq!(
        coach::explain_with(r, |_| Ok("提升99%".into())).kind,
        "fallback"
    );
}

#[test]
fn plateau_affects_plan_with_explanation() {
    let b = bm();
    let rs: Vec<_> = (0..30)
        .map(|i| run("Synthetic Control α", i * 86400, 900.))
        .collect();
    let v = benchmark::view(&b, &rs, "Test Silver", 29 * 86400);
    let p = coach::plan(Some(&v), &Settings::default(), &rs, 29 * 86400);
    assert!(p
        .items
        .iter()
        .find(|i| i.category == "Control Tracking")
        .unwrap()
        .reason
        .contains("平台期"));
}
fn fixture() -> &'static [u8] {
    include_bytes!(
        "../../tests/fixtures/Synthetic Control α - Challenge - 2026.10.01-18.30.00 Stats.csv"
    )
}
fn bm() -> BenchmarkConfig {
    serde_json::from_str(include_str!(
        "../../tests/fixtures/benchmark.synthetic.json"
    ))
    .unwrap()
}
fn run(name: &str, timestamp: i64, score: f64) -> TrainingRun {
    TrainingRun {
        run_id: format!("{name}-{timestamp}"),
        scenario: name.into(),
        timestamp,
        timestamp_basis: "synthetic".into(),
        score,
        accuracy: None,
        kills: None,
        shots: None,
        hits: None,
        duration: None,
        source_file: "synthetic.csv".into(),
        raw: serde_json::Value::Null,
    }
}
fn db() -> (tempfile::TempDir, rusqlite::Connection) {
    let d = tempfile::tempdir().unwrap();
    let c = storage::open(&d.path().join("test.sqlite")).unwrap();
    (d, c)
}
fn write_old(p: &Path, content: &[u8]) {
    fs::write(p, content).unwrap();
    let f = fs::File::options().write(true).open(p).unwrap();
    f.set_times(fs::FileTimes::new().set_modified(SystemTime::now() - Duration::from_secs(10)))
        .unwrap();
}

#[test]
fn real_39_shape_short_weapon_rows_and_time_only_start() {
    let csv=b"Weapon,Shots,Hits,Damage Done,Damage Possible,,Sens Scale,FOV\nTest,100,80,80,100,\n\nScore:,1000\nScenario:,Synthetic Actual Shape\nChallenge Start:,22:02:37.794\nFight Time:,0.0\n";
    let r = KovaakCsvParser
        .parse(
            Path::new("Synthetic Actual Shape - Challenge - 2026.10.03-22.03.37 Stats.csv"),
            csv,
        )
        .unwrap();
    assert_eq!(r.accuracy, Some(0.8));
    assert_eq!(r.duration, None);
    assert_eq!(
        r.timestamp,
        aimcoach_core::parser::parse_time("2026.10.03-22.02.37").unwrap()
    );
    assert_eq!(r.timestamp_basis, "challenge_start_time_with_filename_date");
}
#[test]
fn midnight_start_date_rolls_back_but_same_second_does_not() {
    let csv = b"Score:,50\nScenario:,Synthetic\nChallenge Start:,23:59:40.800\n";
    let r = KovaakCsvParser
        .parse(
            Path::new("Synthetic - Challenge - 2026.10.04-00.00.40 Stats.csv"),
            csv,
        )
        .unwrap();
    assert_eq!(
        r.timestamp,
        aimcoach_core::parser::parse_time("2026.10.03-23.59.40").unwrap()
    );
    let csv = b"Score:,50\nScenario:,Synthetic\nChallenge Start:,22:02:37.794\n";
    let r = KovaakCsvParser
        .parse(
            Path::new("Synthetic - Challenge - 2026.10.03-22.02.37 Stats.csv"),
            csv,
        )
        .unwrap();
    assert_eq!(
        r.timestamp,
        aimcoach_core::parser::parse_time("2026.10.03-22.02.37").unwrap()
    );
}

#[test]
fn parses_multisection_weapon_totals_not_kill_accuracy() {
    let r = KovaakCsvParser
        .parse(Path::new("special α.csv"), fixture())
        .unwrap();
    assert_eq!(r.score, 950.0);
    assert_eq!(r.accuracy, Some(0.8));
    assert_eq!(r.shots, Some(100.0));
    assert_eq!(r.hits, Some(80.0));
    assert_eq!(r.duration, Some(60.0));
    assert_eq!(r.timestamp_basis, "explicit_challenge_start");
    assert!(r.raw["rows"].is_array());
}
#[test]
fn parses_summary_percent_and_offset_time() {
    let r = KovaakCsvParser
        .parse(
            Path::new("summary.csv"),
            include_bytes!("../../tests/fixtures/summary.csv"),
        )
        .unwrap();
    assert_eq!(r.scenario, "Synthetic, click");
    assert_eq!(r.accuracy, Some(0.9));
    assert_eq!(
        r.timestamp,
        aimcoach_core::parser::parse_time("2026-10-02T02:05:00Z").unwrap()
    );
}
#[test]
fn filename_fallback_and_missing_values_stay_null() {
    let r = KovaakCsvParser
        .parse(
            Path::new("Synthetic 场景 - Challenge - 2026.10.01-12.01.00 Stats.csv"),
            b"Score:,123\n",
        )
        .unwrap();
    assert_eq!(r.accuracy, None);
    assert_eq!(r.duration, None);
    assert_eq!(r.kills, None);
    assert_eq!(r.timestamp_basis, "filename_challenge_start");
}
#[test]
fn invalid_csv_and_nan_are_rejected() {
    assert!(KovaakCsvParser
        .parse(
            Path::new("x.csv"),
            include_bytes!("../../tests/fixtures/invalid.csv")
        )
        .is_err());
    assert!(KovaakCsvParser
        .parse(
            Path::new("Synthetic - Challenge - 2026.10.01-12.00.00 Stats.csv"),
            b"Score:,NaN\n"
        )
        .is_err());
    assert!(KovaakCsvParser
        .parse(Path::new("x.csv"), b"Score:,5\n")
        .is_err());
}
#[test]
fn utf16_bom_supported() {
    let text = "Scenario Name:,Synthetic UTF16\nTimestamp:,2026-10-01T12:00:00Z\nScore:,50\n";
    let mut bytes = vec![0xff, 0xfe];
    for n in text.encode_utf16() {
        bytes.extend(n.to_le_bytes());
    }
    assert_eq!(
        KovaakCsvParser
            .parse(Path::new("x.csv"), &bytes)
            .unwrap()
            .score,
        50.0
    );
}
#[test]
fn duplicate_scan_and_copied_file_do_not_duplicate() {
    let (_d, mut c) = db();
    assert!(importer::import_bytes(&mut c, Path::new("one.csv"), fixture(), false).unwrap());
    assert!(!importer::import_bytes(&mut c, Path::new("one.csv"), fixture(), false).unwrap());
    assert!(!importer::import_bytes(&mut c, Path::new("copy α.csv"), fixture(), false).unwrap());
    assert!(!importer::import_bytes(&mut c, Path::new("one.csv"), fixture(), true).unwrap());
    assert_eq!(storage::runs(&c).unwrap().len(), 1);
}
#[test]
fn corrected_file_updates_existing_run() {
    let (_d, mut c) = db();
    importer::import_bytes(&mut c, Path::new("x.csv"), fixture(), false).unwrap();
    let text = String::from_utf8(fixture().to_vec())
        .unwrap()
        .replace("950", "970");
    assert!(!importer::import_bytes(&mut c, Path::new("x.csv"), text.as_bytes(), false).unwrap());
    let rs = storage::runs(&c).unwrap();
    assert_eq!(rs.len(), 1);
    assert_eq!(rs[0].score, 970.0);
}
#[test]
fn database_sorted_by_timestamp_not_import_order() {
    let (_d, mut c) = db();
    importer::import_bytes(
        &mut c,
        Path::new("late.csv"),
        include_bytes!("../../tests/fixtures/summary.csv"),
        false,
    )
    .unwrap();
    importer::import_bytes(&mut c, Path::new("early.csv"), fixture(), false).unwrap();
    let rs = storage::runs(&c).unwrap();
    assert!(rs[0].timestamp < rs[1].timestamp);
}
#[test]
fn scan_isolates_bad_files_and_repeated_scan() {
    let dir = tempfile::tempdir().unwrap();
    let stats = dir.path().join("stats");
    fs::create_dir(&stats).unwrap();
    write_old(&stats.join("α.csv"), fixture());
    write_old(
        &stats.join("invalid.csv"),
        include_bytes!("../../tests/fixtures/invalid.csv"),
    );
    let p = dir.path().join("db.sqlite");
    let a = importer::scan(&p, &stats, false).unwrap();
    assert_eq!((a.imported, a.failed), (1, 1));
    let b = importer::scan(&p, &stats, false).unwrap();
    assert_eq!((b.imported, b.skipped, b.failed), (0, 1, 1));
    assert_eq!(storage::runs(&storage::open(&p).unwrap()).unwrap().len(), 1);
}
#[test]
fn newly_written_file_is_deferred() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("x.csv");
    fs::write(&p, fixture()).unwrap();
    assert!(importer::stable_read(&p).unwrap().is_none());
}
#[test]
fn pb_up_median_down_is_not_stable_progress() {
    let values = [930., 950., 940., 1000., 960., 870., 860., 900., 1050., 880.];
    let rs: Vec<_> = values
        .iter()
        .enumerate()
        .map(|(i, v)| run("A", i as i64, *v))
        .collect();
    let a = analysis::scenario("A", &rs, 20);
    assert_eq!(a.verdict, "pb_only");
    assert!(a.pb_change.unwrap() > 0.0);
    assert!(a.median_change.unwrap() < 0.0);
}
#[test]
fn four_plus_four_is_insufficient_even_with_pb() {
    let values = [930., 950., 940., 1000., 870., 860., 900., 1050.];
    let rs: Vec<_> = values
        .iter()
        .enumerate()
        .map(|(i, v)| run("A", i as i64, *v))
        .collect();
    assert_eq!(analysis::scenario("A", &rs, 20).verdict, "insufficient");
}
#[test]
fn sustained_improvement_is_detected() {
    let values = [100., 101., 100., 99., 100., 110., 111., 110., 109., 110.];
    let rs: Vec<_> = values
        .iter()
        .enumerate()
        .map(|(i, v)| run("A", i as i64, *v))
        .collect();
    assert_eq!(analysis::scenario("A", &rs, 20).verdict, "improved");
}
#[test]
fn insufficient_data_has_no_invented_conclusion() {
    let a = analysis::scenario("A", &[], 0);
    assert_eq!(a.verdict, "insufficient");
    assert_eq!(a.pb, None);
    assert_eq!(a.days7.percent, None);
    assert!(!a.plateau);
}
#[test]
fn threshold_boundaries_and_intra_rank_progress() {
    let b = bm();
    benchmark::validate(&b).unwrap();
    let s = &b.scenarios[0];
    let l = benchmark::level(1250., s, &b.ranks);
    assert_eq!(l.rank.as_deref(), Some("Test Silver"));
    assert_eq!(l.progress, 0.5);
    assert_eq!(l.normalized, 2.5);
    assert_eq!(benchmark::level(499., s, &b.ranks).rank, None);
    assert_eq!(benchmark::level(1000., s, &b.ranks).progress, 0.0);
    assert_eq!(benchmark::level(1600., s, &b.ranks).next_rank, None);
}
#[test]
fn distinct_raw_scores_are_only_compared_after_normalization() {
    let b = bm();
    let rs = vec![
        run("Synthetic Control α", 1, 1000.),
        run("Synthetic Reactive", 2, 100.),
    ];
    let v = benchmark::view(&b, &rs, "Test Silver", 5);
    assert_eq!(
        v.categories
            .iter()
            .find(|c| c.name == "Control Tracking")
            .unwrap()
            .mean,
        Some(2.0)
    );
    assert_eq!(
        v.categories
            .iter()
            .find(|c| c.name == "Reactive Tracking")
            .unwrap()
            .mean,
        Some(2.0)
    );
    assert_eq!(v.complete[1].reached, 2);
    assert_eq!(v.complete[1].total, 4);
}
#[test]
fn group_uses_all_scenarios_not_highest() {
    let mut b = bm();
    let mut second = b.scenarios[0].clone();
    second.name = "Synthetic Control weak".into();
    second.aliases.clear();
    b.scenarios.push(second);
    benchmark::validate(&b).unwrap();
    let rs = vec![
        run("Synthetic Control α", 1, 1500.),
        run("Synthetic Control weak", 2, 500.),
    ];
    let v = benchmark::view(&b, &rs, "Test Silver", 5);
    let c = v
        .categories
        .iter()
        .find(|c| c.name == "Control Tracking")
        .unwrap();
    assert_eq!(c.mean, Some(2.0));
    assert_eq!(c.best, Some(3.0));
    assert_eq!(c.lowest, Some(1.0));
}
#[test]
fn unknown_benchmark_scenario_remains_visible_without_rank() {
    let rs = vec![run("Unknown", 1, 123.)];
    let a = analysis::all(&rs, 5);
    assert_eq!(a[0].pb, Some(123.));
    let v = benchmark::view(&bm(), &rs, "Test Silver", 5);
    assert_eq!(v.unmatched, vec!["Unknown"]);
    assert!(v.rows.iter().all(|r| r.best.is_none()));
}
#[test]
fn bad_config_rejected_aliases_match() {
    let mut b = bm();
    assert!(benchmark::matches(
        &b.scenarios[0],
        "  SYNTHETIC Control alias "
    ));
    b.scenarios[0].thresholds.insert("Test Silver".into(), 400.);
    assert!(benchmark::validate(&b).is_err());
    let mut b = bm();
    b.scenarios[0].aliases.push("Synthetic Reactive".into());
    assert!(benchmark::validate(&b).is_err());
}
#[test]
fn benchmark_roundtrip_and_migration_preserve_data() {
    let (d, mut c) = db();
    importer::import_bytes(&mut c, Path::new("x.csv"), fixture(), false).unwrap();
    storage::save_benchmark(&mut c, &bm()).unwrap();
    drop(c);
    let c = storage::open(&d.path().join("test.sqlite")).unwrap();
    assert_eq!(storage::runs(&c).unwrap().len(), 1);
    assert_eq!(storage::benchmark(&c).unwrap().unwrap().ranks.len(), 3);
}
#[test]
fn empty_db_offline_report_and_plan_work() {
    let (_d, c) = db();
    let s = storage::settings(&c).unwrap();
    let rs = storage::runs(&c).unwrap();
    let r = coach::local_report(&rs, None, &s, 0);
    assert!(r.text.contains("尚无训练记录"));
    assert_eq!(r.kind, "local");
    assert!(coach::plan(None, &s, &rs, 0).items.is_empty());
}
#[test]
fn api_network_failure_falls_back_to_local_report() {
    let mut s = Settings::default();
    s.ai_consent = true;
    s.ai_model = "synthetic-model".into();
    s.ai_base_url = "http://127.0.0.1:1/v1".into();
    let local = coach::local_report(&[], None, &s, 0);
    let r = coach::explain_with(local, |ev| coach::api_explain(ev, &s, "synthetic-test-key"));
    assert_eq!(r.kind, "fallback");
    assert!(r.text.contains("尚无训练记录"));
    assert!(r.text.contains("AI 调用失败"));
    assert!(!r.evidence.to_string().contains("synthetic-test-key"));
}
#[test]
fn no_ai_request_without_consent() {
    let mut s = Settings::default();
    s.ai_base_url = "http://127.0.0.1:1/v1".into();
    assert!(coach::api_explain(&serde_json::json!({}), &s, "synthetic")
        .unwrap_err()
        .contains("授权"));
}
#[test]
fn report_evidence_omits_paths_and_raw_rows() {
    let r = KovaakCsvParser
        .parse(Path::new("secret-user-path.csv"), fixture())
        .unwrap();
    let report = coach::local_report(&[r], None, &Settings::default(), 0);
    let j = report.evidence.to_string();
    assert!(!j.contains("secret-user-path"));
    assert!(!j.contains("raw"));
    assert!(!j.contains("source_file"));
}
#[test]
fn plan_minutes_sum_and_scenarios_are_from_config() {
    let b = bm();
    let v = benchmark::view(&b, &[], "Test Silver", 0);
    let mut s = Settings::default();
    s.daily_minutes = 61;
    s.focus = "Reactive Tracking".into();
    let p = coach::plan(Some(&v), &s, &[], 0);
    assert_eq!(p.items.iter().map(|x| x.minutes).sum::<u32>(), 61);
    assert!(p
        .items
        .iter()
        .flat_map(|x| &x.scenarios)
        .all(|n| b.scenarios.iter().any(|s| &s.name == n)));
    assert!(
        p.items
            .iter()
            .find(|x| x.category == s.focus)
            .unwrap()
            .minutes
            > p.items
                .iter()
                .find(|x| x.category == "Flick Tech")
                .unwrap()
                .minutes
    );
}
#[test]
fn session_rule_and_fatigue_compare_same_scenario() {
    let rs: Vec<_> = [100., 100., 100., 95., 95., 95., 90., 90., 90.]
        .iter()
        .enumerate()
        .map(|(i, v)| run("A", i as i64 * 60, *v))
        .chain(std::iter::once(run("B", 2000, 99999.)))
        .collect();
    let sessions = analysis::sessions(&rs, 20);
    assert_eq!(sessions.len(), 2);
    assert_eq!(sessions[0].decay.len(), 1);
    assert_eq!(sessions[1].decay.len(), 0);
}
#[test]
fn plateau_requires_duration_and_samples() {
    let rs: Vec<_> = (0..30).map(|i| run("A", i * 86400, 100.)).collect();
    assert!(analysis::scenario("A", &rs, 29 * 86400).plateau);
    let short: Vec<_> = (0..30).map(|i| run("A", i * 60, 100.)).collect();
    assert!(!analysis::scenario("A", &short, 29 * 60).plateau);
}
#[test]
fn regroup_uses_configurable_gap() {
    let (_d, mut c) = db();
    for (i, time) in [
        "2026-10-01T12:00:00Z",
        "2026-10-01T12:15:00Z",
        "2026-10-01T12:50:00Z",
    ]
    .iter()
    .enumerate()
    {
        let csv = format!("Scenario Name:,A\nTimestamp:,{time}\nScore:,{}\n", i + 1);
        importer::import_bytes(
            &mut c,
            Path::new(&format!("{i}.csv")),
            csv.as_bytes(),
            false,
        )
        .unwrap();
    }
    storage::regroup(&mut c, 20).unwrap();
    assert_eq!(
        c.query_row::<i64, _, _>("SELECT COUNT(*) FROM training_session", [], |r| r.get(0))
            .unwrap(),
        2
    );
    storage::regroup(&mut c, 10).unwrap();
    assert_eq!(
        c.query_row::<i64, _, _>("SELECT COUNT(*) FROM training_session", [], |r| r.get(0))
            .unwrap(),
        3
    );
}
