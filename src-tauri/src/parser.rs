//! Only explicit summary fields or weapon totals are used. Kill-table accuracy is
//! not a run accuracy. Filename timestamps are local challenge START times.
use crate::model::TrainingRun;
use chrono::{DateTime, Local, NaiveDateTime, NaiveTime, TimeZone};
use regex::Regex;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::Path};

pub trait RunParser: Send + Sync {
    fn parse(&self, path: &Path, bytes: &[u8]) -> Result<TrainingRun, String>;
}
pub struct KovaakCsvParser;
fn key(s: &str) -> String {
    s.trim()
        .trim_end_matches(':')
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}
fn decode(bytes: &[u8]) -> Result<String, String> {
    if bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]) {
        if bytes.len() % 2 != 0 {
            return Err("UTF-16 文件尚未写完或损坏".into());
        }
        let little = bytes[0] == 0xff;
        let words: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|x| {
                if little {
                    u16::from_le_bytes([x[0], x[1]])
                } else {
                    u16::from_be_bytes([x[0], x[1]])
                }
            })
            .collect();
        String::from_utf16(&words).map_err(|_| "无效 UTF-16 编码".into())
    } else {
        String::from_utf8(bytes.to_vec())
            .map(|s| s.trim_start_matches('\u{feff}').to_owned())
            .map_err(|_| "不支持的编码：需要 UTF-8 或带 BOM 的 UTF-16".into())
    }
}
fn number(s: &str) -> Result<f64, String> {
    let v = s
        .trim()
        .trim_end_matches('%')
        .trim_end_matches('s')
        .trim()
        .parse::<f64>()
        .map_err(|_| format!("无效数值：{s}"))?;
    if !v.is_finite() || v < 0.0 {
        return Err(format!("数值必须有限且非负：{s}"));
    }
    Ok(v)
}
pub fn parse_time(s: &str) -> Result<i64, String> {
    if let Ok(v) = DateTime::parse_from_rfc3339(s.trim()) {
        return Ok(v.timestamp());
    }
    for fmt in [
        "%Y.%m.%d-%H.%M.%S",
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%d-%H-%M-%S",
        "%Y/%m/%d %H:%M:%S",
        "%Y.%m.%d-%H:%M:%S",
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y.%m.%d-%H.%M.%S%.f",
    ] {
        if let Ok(v) = NaiveDateTime::parse_from_str(s.trim(), fmt) {
            return Local
                .from_local_datetime(&v)
                .single()
                .map(|d| d.timestamp())
                .ok_or_else(|| "本地时间在夏令时转换处存在歧义".into());
        }
    }
    Err(format!("无法识别时间戳：{s}（不会使用文件修改时间替代）"))
}
impl RunParser for KovaakCsvParser {
    fn parse(&self, path: &Path, bytes: &[u8]) -> Result<TrainingRun, String> {
        let text = decode(bytes)?;
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(true)
            .trim(csv::Trim::All)
            .from_reader(text.as_bytes());
        let rows: Vec<Vec<String>> = reader
            .records()
            .map(|r| {
                r.map(|x| x.iter().map(str::to_owned).collect())
                    .map_err(|e| format!("CSV 语法错误：{e}"))
            })
            .collect::<Result<_, _>>()?;
        let mut fields = BTreeMap::<String, String>::new();
        let mut provenance = BTreeMap::<String, String>::new();
        let summary_keys = [
            "score",
            "kills",
            "accuracy",
            "fighttime",
            "duration",
            "challengeduration",
            "shotsfired",
            "shotshit",
            "shots",
            "hits",
            "hitcount",
            "misscount",
            "scenarioname",
            "scenario",
            "challengestart",
            "challengestarttime",
            "timestamp",
            "completedat",
            "completiontime",
        ];
        // Known multi-section files have arbitrary-length rows and key/value summaries.
        for row in &rows {
            if row.len() >= 2
                && summary_keys.contains(&key(&row[0]).as_str())
                && (row.len() == 2 || row[2..].iter().all(|s| s.is_empty()))
            {
                let k = key(&row[0]);
                if fields.contains_key(&k) {
                    return Err(format!("重复摘要字段：{}", row[0]));
                }
                if !row[1].is_empty() {
                    fields.insert(k.clone(), row[1].clone());
                    provenance.insert(k, "summary".into());
                }
            }
        }
        // Explicit one-row summary exports (not per-kill rows).
        for (i, row) in rows.iter().enumerate() {
            let keys: Vec<String> = row.iter().map(|s| key(s)).collect();
            if keys.contains(&"score".into())
                && (keys.contains(&"scenarioname".into()) || keys.contains(&"scenario".into()))
            {
                if let Some(values) = rows.get(i + 1) {
                    if values.len() == keys.len() {
                        for (k, v) in keys.iter().zip(values) {
                            if summary_keys.contains(&k.as_str()) && !v.is_empty() {
                                fields.insert(k.clone(), v.clone());
                                provenance.insert(k.clone(), "summary_table".into());
                            }
                        }
                    }
                }
                if rows
                    .get(i + 2)
                    .is_some_and(|r| r.iter().any(|v| !v.is_empty()))
                {
                    return Err("单次训练 CSV 中存在多条摘要记录；请使用一训练一文件格式".into());
                }
            }
        }
        // Weapon aggregate section is reliable for total shots/hits; never sum kill rows.
        if !fields.contains_key("shots") && !fields.contains_key("shotsfired") {
            for (i, row) in rows.iter().enumerate() {
                let ks: Vec<String> = row.iter().map(|s| key(s)).collect();
                if ks.first().is_some_and(|s| s == "weapon") {
                    if let (Some(si), Some(hi)) = (
                        ks.iter().position(|x| x == "shots"),
                        ks.iter().position(|x| x == "hits"),
                    ) {
                        let (mut shots, mut hits, mut n) = (0.0, 0.0, 0);
                        for r in rows
                            .iter()
                            .skip(i + 1)
                            .take_while(|r| r.len() > si.max(hi) && !r[0].is_empty())
                        {
                            shots += number(&r[si])?;
                            hits += number(&r[hi])?;
                            n += 1;
                        }
                        if n > 0 {
                            fields.insert("shots".into(), shots.to_string());
                            fields.insert("hits".into(), hits.to_string());
                            provenance.insert("shots".into(), "weapon_totals".into());
                            provenance.insert("hits".into(), "weapon_totals".into());
                        }
                    }
                }
            }
        }
        let filename = path
            .file_name()
            .and_then(|x| x.to_str())
            .ok_or("文件名不是有效 Unicode")?;
        let re = Regex::new(r"(?i)^(.*) - Challenge - (.+) Stats\.csv$").unwrap();
        let cap = re.captures(filename);
        let scenario = fields
            .get("scenarioname")
            .or_else(|| fields.get("scenario"))
            .cloned()
            .or_else(|| cap.as_ref().map(|c| c[1].to_owned()))
            .filter(|s| !s.trim().is_empty())
            .ok_or("缺少场景名称，且文件名不符合 Challenge 格式")?;
        let (time, basis) = if let Some(t) = fields
            .get("completedat")
            .or_else(|| fields.get("completiontime"))
        {
            (t.clone(), "explicit_completion")
        } else if let Some(t) = fields
            .get("challengestarttime")
            .or_else(|| fields.get("challengestart"))
        {
            (t.clone(), "explicit_challenge_start")
        } else if let Some(t) = fields.get("timestamp") {
            (t.clone(), "explicit_timestamp")
        } else if let Some(c) = cap {
            (c[2].to_owned(), "filename_challenge_start")
        } else {
            return Err("缺少可验证的训练时间".into());
        };
        // 3.9 exports a time-only Challenge Start. Use the filename's date,
        // rolling back one day across midnight, rather than filesystem metadata.
        let (timestamp, basis) = if let Ok(t) = parse_time(&time) {
            (t, basis.to_owned())
        } else if let (Ok(clock), Some(c)) = (
            NaiveTime::parse_from_str(&time, "%H:%M:%S%.f"),
            re.captures(filename),
        ) {
            let file_time = parse_time(&c[2])?;
            let local = Local
                .timestamp_opt(file_time, 0)
                .single()
                .ok_or("文件名日期无效")?;
            let date = if clock.signed_duration_since(local.time()).num_hours() > 12 {
                local.date_naive().pred_opt().ok_or("日期下溢")?
            } else {
                local.date_naive()
            };
            let dt = Local
                .from_local_datetime(&date.and_time(clock))
                .single()
                .ok_or("本地时间存在歧义")?;
            (
                dt.timestamp(),
                "challenge_start_time_with_filename_date".to_owned(),
            )
        } else {
            return Err(format!(
                "无法识别时间戳：{time}，或缺少用于日期的 Challenge 文件名"
            ));
        };
        let optional = |aliases: &[&str]| -> Result<Option<f64>, String> {
            aliases
                .iter()
                .find_map(|k| fields.get(*k))
                .map(|v| number(v))
                .transpose()
        };
        let score = optional(&["score"])?.ok_or("缺少训练摘要 Score（不从击杀表推测）")?;
        let shots = optional(&["shotsfired", "shots"])?;
        let hits = optional(&["shotshit", "hits", "hitcount"])?;
        if let (Some(s), Some(h)) = (shots, hits) {
            if h > s {
                return Err("命中数大于射击数".into());
            }
        }
        let mut accuracy = optional(&["accuracy"])?;
        if let Some(v) = accuracy {
            let original = fields.get("accuracy").unwrap();
            accuracy = Some(if original.contains('%') || v > 1.0 {
                v / 100.0
            } else {
                v
            });
            if accuracy.unwrap() > 1.0 {
                return Err("Accuracy 超出 0–100%".into());
            }
        }
        if accuracy.is_none() {
            if let (Some(s), Some(h)) = (shots, hits) {
                if s > 0.0 {
                    accuracy = Some(h / s);
                    provenance.insert("accuracy".into(), "derived_hits_over_shots".into());
                }
            }
        }
        let id = format!(
            "{:x}",
            Sha256::digest(format!("kovaak\0{}\0{timestamp}", scenario.trim()).as_bytes())
        );
        // Fight Time is combat time (can be 0 for tracking); not challenge duration.
        Ok(TrainingRun {
            run_id: id,
            scenario,
            timestamp,
            timestamp_basis: basis,
            score,
            accuracy,
            kills: optional(&["kills"])?,
            shots,
            hits,
            duration: optional(&["challengeduration", "duration"])?.filter(|v| *v > 0.0),
            source_file: path.to_string_lossy().into(),
            raw: serde_json::json!({"rows":rows,"fields":fields,"provenance":provenance,"parser_version":2}),
        })
    }
}
