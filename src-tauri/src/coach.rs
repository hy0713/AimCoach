use crate::{
    analysis,
    benchmark::{self, BenchmarkView},
    model::*,
};
use chrono::{Local, TimeZone};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Today {
    pub date: String,
    pub count: usize,
    pub duration_seconds: f64,
    pub duration_known: usize,
    pub pb_count: usize,
    pub scenarios: Vec<String>,
    pub category_counts: BTreeMap<String, usize>,
}
pub fn today(runs: &[TrainingRun], b: Option<&BenchmarkConfig>, now: i64) -> Today {
    let date = Local
        .timestamp_opt(now, 0)
        .single()
        .unwrap()
        .format("%Y-%m-%d")
        .to_string();
    let rs: Vec<_> = runs
        .iter()
        .filter(|r| {
            Local
                .timestamp_opt(r.timestamp, 0)
                .single()
                .is_some_and(|d| d.format("%Y-%m-%d").to_string() == date)
        })
        .collect();
    let mut pb_count = 0;
    let mut previous: BTreeMap<String, f64> = BTreeMap::new();
    let mut sorted: Vec<_> = runs.iter().collect();
    sorted.sort_by_key(|r| r.timestamp);
    for r in sorted {
        if let Some(old) = previous.get(&r.scenario) {
            if r.score > *old && rs.iter().any(|x| x.run_id == r.run_id) {
                pb_count += 1;
            }
        }
        previous
            .entry(r.scenario.clone())
            .and_modify(|v| *v = v.max(r.score))
            .or_insert(r.score);
    }
    let scenarios = rs
        .iter()
        .map(|r| r.scenario.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut category_counts = BTreeMap::new();
    for r in &rs {
        let cat = b
            .and_then(|b| {
                b.scenarios
                    .iter()
                    .find(|s| benchmark::matches(s, &r.scenario))
            })
            .map(|s| s.category.clone())
            .unwrap_or("未分类".into());
        *category_counts.entry(cat).or_default() += 1;
    }
    Today {
        date,
        count: rs.len(),
        duration_seconds: rs.iter().filter_map(|r| r.duration).sum(),
        duration_known: rs.iter().filter(|r| r.duration.is_some()).count(),
        pb_count,
        scenarios,
        category_counts,
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanItem {
    pub category: String,
    pub minutes: u32,
    pub scenarios: Vec<String>,
    pub reason: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub total_minutes: u32,
    pub items: Vec<PlanItem>,
    pub notes: Vec<String>,
}
pub fn plan(
    b: Option<&BenchmarkView>,
    settings: &Settings,
    runs: &[TrainingRun],
    now: i64,
) -> Plan {
    let Some(b) = b else {
        return Plan {
            total_minutes: settings.daily_minutes,
            items: Vec::new(),
            notes: vec![
                "请先导入包含真实场景和门槛的 Benchmark JSON，才能生成场景训练计划。".into(),
            ],
        };
    };
    let mut weights = Vec::new();
    let plateau_names: std::collections::BTreeSet<_> = analysis::all(runs, now)
        .into_iter()
        .filter(|a| a.plateau)
        .map(|a| a.name)
        .collect();
    let overall: Vec<_> = b.categories.iter().filter_map(|c| c.mean).collect();
    let mean = if overall.is_empty() {
        0.0
    } else {
        overall.iter().sum::<f64>() / overall.len() as f64
    };
    for c in &b.categories {
        let mut weight = 1.0;
        let mut reasons = Vec::new();
        if c.covered < c.total {
            weight += 0.6;
            reasons.push("部分场景缺记录，优先建立基线");
        }
        if settings.focus == c.name {
            weight += 1.5;
            reasons.push("你设置的主攻大项");
        }
        if settings.prioritize_weak && c.mean.is_some_and(|v| v < mean) {
            weight += 1.0;
            reasons.push("近期中位数归一化表现低于个人大类均值");
        }
        if c.change.is_some_and(|v| v.abs() < 1.0) {
            weight += 0.3;
            reasons.push("近期变化较小，调整练习组合");
        }
        if b.rows
            .iter()
            .filter(|r| r.category == c.name)
            .any(|r| r.matched_names.iter().any(|n| plateau_names.contains(n)))
        {
            weight += 0.5;
            reasons.push("存在满足样本量与跨度规则的平台期场景，建议轮换相关子项");
        }
        if !settings.maintain_strength
            && c.mean.is_some_and(|v| v > mean)
            && settings.focus != c.name
        {
            weight *= 0.35;
        }
        let overdue = b.rows.iter().filter(|r| r.category == c.name).all(|r| {
            runs.iter()
                .filter(|x| r.matched_names.contains(&x.scenario))
                .map(|x| x.timestamp)
                .max()
                .is_none_or(|t| now - t > 7 * 86400)
        });
        if overdue {
            weight += 0.4;
            reasons.push("超过 7 天未训练或尚无记录");
        }
        if reasons.is_empty() {
            reasons.push("保留常规练习，观察稳定表现");
        }
        weights.push((c.name.clone(), weight, reasons.join("；")));
    }
    let sum: f64 = weights.iter().map(|w| w.1).sum();
    let mut items = Vec::new();
    let mut assigned = 0;
    for (cat, w, reason) in weights {
        let minutes = (settings.daily_minutes as f64 * w / sum).floor() as u32;
        assigned += minutes;
        let mut candidates: Vec<_> = b.rows.iter().filter(|r| r.category == cat).collect();
        candidates.sort_by(|a, b| {
            a.target_met
                .cmp(&b.target_met)
                .then_with(|| {
                    a.matched_names
                        .iter()
                        .any(|n| plateau_names.contains(n))
                        .cmp(&b.matched_names.iter().any(|n| plateau_names.contains(n)))
                })
                .then_with(|| {
                    a.stable
                        .as_ref()
                        .map(|l| l.normalized)
                        .unwrap_or(-1.0)
                        .total_cmp(&b.stable.as_ref().map(|l| l.normalized).unwrap_or(-1.0))
                })
        });
        items.push(PlanItem {
            category: cat,
            minutes,
            scenarios: candidates
                .into_iter()
                .take(3)
                .map(|r| r.name.clone())
                .collect(),
            reason,
        });
    }
    let remaining = settings.daily_minutes - assigned;
    for i in 0..remaining {
        let n = items.len();
        items[i as usize % n].minutes += 1;
    }
    Plan {
        total_minutes: settings.daily_minutes,
        items,
        notes: vec![
            format!(
                "目标：{} Complete。建议时长由规则分配，未自动修改游戏设置。",
                b.target
            ),
            "建议按场景分成短组，组间休息；总时长包含休息安排。".into(),
        ],
    }
}
pub fn local_report(
    runs: &[TrainingRun],
    b: Option<&BenchmarkConfig>,
    s: &Settings,
    now: i64,
) -> Report {
    let t = today(runs, b, now);
    let all = analysis::all(runs, now);
    let today_names = &t.scenarios;
    let mut lines = vec![
        format!("{} · 本地训练总结", t.date),
        format!(
            "今日记录 {} 次训练，覆盖 {} 个场景；刷新已有 PB {} 次。",
            t.count,
            t.scenarios.len(),
            t.pb_count
        ),
    ];
    if t.count == 0 {
        lines
            .push("今天尚无训练记录。可以扫描 Stats 历史目录；不会根据空数据生成进步判断。".into());
    }
    if t.duration_known > 0 {
        lines.push(format!(
            "有时长的 {} 次记录累计 {:.1} 分钟；缺失时长的记录未估算。",
            t.duration_known,
            t.duration_seconds / 60.0
        ));
    }
    for (cat, n) in &t.category_counts {
        lines.push(format!("{cat}：{n} 次有效记录。"));
    }
    for a in &all {
        if today_names.contains(&a.name) {
            lines.push(format!("{}：{}", a.name, a.explanation));
            if a.plateau {
                lines.push("近期 30 天至少 30 次、跨度至少 21 天，中位数变化小于 1% 且波动较低：可能进入平台期，建议相关场景轮换。".into());
            }
            if a.anomalies > 0 {
                lines.push(format!(
                    "最近 10 次中有 {} 次偏离较大；先观察后续数据，不能据此推断动作缺陷。",
                    a.anomalies
                ));
            }
        }
    }
    let bv = b.map(|b| benchmark::view(b, runs, &s.target_rank, now));
    let p = plan(bv.as_ref(), s, runs, now);
    for item in &p.items {
        lines.push(format!(
            "下次 {}：{} 分钟。理由：{}。",
            item.category, item.minutes, item.reason
        ));
    }
    if p.items.is_empty() {
        lines.extend(p.notes.clone());
    }
    let sessions = analysis::sessions(runs, s.session_gap_minutes);
    if let Some(session) = sessions.last() {
        if session.end >= now - 86400 {
            lines.extend(session.decay.clone());
        }
    }
    lines.push("所有判断是个人成绩表现分析；CSV 汇总记录不足以判断 overshoot、手腕控制、Y 轴控制或医学疲劳。".into());
    // No paths, raw rows, or API credentials go into the evidence sent to a model.
    Report {
        id: 0,
        generated_at: now,
        date: t.date.clone(),
        text: lines.join("\n\n"),
        kind: "local".into(),
        evidence: serde_json::json!({"today":t,"scenarios":all,"skills":bv.as_ref().map(|v|&v.categories),"plan":p,"session_decay":sessions.last().map(|v|&v.decay)}),
    }
}
/// Numeric guard only: natural language claims still require comparison with evidence.
pub fn valid_ai_percentages(text: &str, evidence: &serde_json::Value) -> bool {
    fn collect(value: &serde_json::Value, out: &mut Vec<f64>) {
        match value {
            serde_json::Value::Object(map) => {
                for (key, v) in map {
                    if [
                        "percent",
                        "median_change",
                        "pb_change",
                        "variability",
                        "previous_variability",
                        "accuracy_change",
                        "recent_change",
                        "change",
                    ]
                    .contains(&key.as_str())
                    {
                        if let Some(n) = v.as_f64() {
                            out.push(n.abs());
                        }
                    }
                    collect(v, out);
                }
            }
            serde_json::Value::Array(values) => {
                for v in values {
                    collect(v, out);
                }
            }
            _ => {}
        }
    }
    let mut allowed = Vec::new();
    collect(evidence, &mut allowed);
    let regex = regex::Regex::new(r"([-+]?\d+(?:\.\d+)?)\s*(?:%|％|个百分点)").unwrap();
    let valid = regex.captures_iter(text).all(|c| {
        c[1].parse::<f64>()
            .is_ok_and(|n| allowed.iter().any(|a| (n.abs() - a).abs() <= 0.11))
    });
    valid
}
pub fn explain_with<F>(mut report: Report, request: F) -> Report
where
    F: FnOnce(&serde_json::Value) -> Result<String, String>,
{
    match request(&report.evidence) {
        Ok(text) if !text.trim().is_empty() && valid_ai_percentages(&text, &report.evidence) => {
            report.text = format!(
                "{}\n\n—— AI 补充解释（可能出错，请对照下方确定性证据）——\n\n{}",
                report.text, text
            );
            report.kind = "ai".into();
        }
        _ => {
            report
                .text
                .push_str("\n\nAI 调用失败，已保留完整本地规则报告。");
            report.kind = "fallback".into();
        }
    }
    report
}
pub fn api_explain(
    evidence: &serde_json::Value,
    s: &Settings,
    key: &str,
) -> Result<String, String> {
    if !s.ai_consent {
        return Err("尚未授权聚合数据离开本机".into());
    }
    s.validate()?;
    if s.ai_model.trim().is_empty() {
        return Err("请填写 Model".into());
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(35))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "无法建立 AI 客户端".to_string())?;
    let response=client.post(format!("{}/chat/completions",s.ai_base_url.trim_end_matches('/'))).bearer_auth(key).json(&serde_json::json!({"model":s.ai_model,"temperature":0.2,"max_tokens":1600,"messages":[{"role":"system","content":"你是中文练枪教练。仅解释给定聚合证据，不计算新百分比，不杜撰记录、场景、动作缺陷或医学判断。样本不足要明确说明。不得将单次 PB 视为稳定进步。简洁、有证据地肯定和指出短板。"},{"role":"user","content":evidence.to_string()}]})).send().map_err(|_|"AI 网络请求失败".to_string())?;
    if !response.status().is_success() {
        return Err(format!("AI HTTP {}", response.status().as_u16()));
    }
    let data: serde_json::Value = response.json().map_err(|_| "AI 返回格式错误".to_string())?;
    data["choices"][0]["message"]["content"]
        .as_str()
        .map(str::to_owned)
        .ok_or("AI 响应缺少文本".into())
}
