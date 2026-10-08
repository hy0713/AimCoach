use crate::model::TrainingRun;
use serde::{Deserialize, Serialize};

pub fn median(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut v = values.to_vec();
    v.sort_by(f64::total_cmp);
    let n = v.len();
    Some(if n % 2 == 0 {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    } else {
        v[n / 2]
    })
}
pub fn change(old: Option<f64>, new: Option<f64>) -> Option<f64> {
    match (old, new) {
        (Some(a), Some(b)) if a > 0.0 => Some((b - a) / a * 100.0),
        _ => None,
    }
}
pub fn cv(v: &[f64]) -> Option<f64> {
    if v.len() < 2 {
        return None;
    }
    let mean = v.iter().sum::<f64>() / v.len() as f64;
    if mean <= 0.0 {
        return None;
    }
    Some((v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / v.len() as f64).sqrt() / mean * 100.0)
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowChange {
    pub previous_count: usize,
    pub recent_count: usize,
    pub percent: Option<f64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioAnalysis {
    pub name: String,
    pub count: usize,
    pub pb: Option<f64>,
    pub latest: Option<f64>,
    pub median10: Option<f64>,
    pub previous_median: Option<f64>,
    pub median_change: Option<f64>,
    pub pb_change: Option<f64>,
    pub variability: Option<f64>,
    pub previous_variability: Option<f64>,
    pub last_trained: Option<i64>,
    pub accuracy_change: Option<f64>,
    pub days7: WindowChange,
    pub days30: WindowChange,
    pub verdict: String,
    pub explanation: String,
    pub plateau: bool,
    pub anomalies: usize,
    pub recent_count: usize,
    pub previous_count: usize,
}
fn window(rs: &[&TrainingRun], now: i64, days: i64) -> WindowChange {
    let a: Vec<f64> = rs
        .iter()
        .filter(|r| r.timestamp >= now - days * 86400 * 2 && r.timestamp < now - days * 86400)
        .map(|r| r.score)
        .collect();
    let b: Vec<f64> = rs
        .iter()
        .filter(|r| r.timestamp >= now - days * 86400 && r.timestamp <= now)
        .map(|r| r.score)
        .collect();
    WindowChange {
        previous_count: a.len(),
        recent_count: b.len(),
        percent: if a.len() >= 5 && b.len() >= 5 {
            change(median(&a), median(&b))
        } else {
            None
        },
    }
}
pub fn scenario(name: &str, runs: &[TrainingRun], now: i64) -> ScenarioAnalysis {
    let mut rs: Vec<&TrainingRun> = runs.iter().filter(|r| r.scenario == name).collect();
    rs.sort_by_key(|r| r.timestamp);
    let n = rs.len();
    let size = 10.min(n / 2);
    let split = n.saturating_sub(size);
    let recent: Vec<f64> = rs.iter().skip(split).map(|r| r.score).collect();
    let past: Vec<f64> = rs
        .iter()
        .skip(split.saturating_sub(size))
        .take(size)
        .map(|r| r.score)
        .collect();
    let latest10: Vec<f64> = rs
        .iter()
        .skip(n.saturating_sub(10))
        .map(|r| r.score)
        .collect();
    let delta = change(median(&past), median(&recent));
    let variability = cv(&latest10);
    let past_cv = cv(&past);
    let pb = rs.iter().map(|r| r.score).reduce(f64::max);
    let old_pb = rs.iter().take(split).map(|r| r.score).reduce(f64::max);
    let pb_delta = change(old_pb, pb);
    let sustained = size >= 5
        && median(&recent[..size / 2])
            .zip(median(&past))
            .is_some_and(|(a, b)| a > b * 1.02)
        && median(&recent[size / 2..])
            .zip(median(&past))
            .is_some_and(|(a, b)| a > b * 1.02);
    let stable_cv = cv(&recent)
        .zip(past_cv)
        .is_some_and(|(a, b)| a <= b * 1.15 + 1.0);
    let (verdict, explanation) = if size < 5 {
        (
            "insufficient",
            "数据不足：需要至少 10 次成绩，前后各 5 次，才能评估稳定表现。".to_string(),
        )
    } else if pb_delta.is_some_and(|v| v > 0.0) && delta.is_some_and(|v| v < -2.0) {
        ("pb_only",format!("PB 上升 {:.1}%，近期中位数下降 {:.1}%。只能确认最高成绩刷新，不能确认稳定水平提升。",pb_delta.unwrap(),-delta.unwrap()))
    } else if delta.is_some_and(|v| v > 2.0) && sustained && stable_cv {
        ("improved",format!("前后各 {size} 次比较：中位数提升 {:.1}%，近期两段均高于过去水平，波动未明显扩大。支持近期稳定表现改善。",delta.unwrap()))
    } else if delta.is_some_and(|v| v < -2.0) {
        ("declined",format!("前后各 {size} 次比较：中位数下降 {:.1}%。建议观察休息后表现与训练组合；CSV 不能确定动作原因。",-delta.unwrap()))
    } else {
        (
            "uncertain",
            "尚无足够的持续改善证据；最高成绩、近期中位数与波动需要一起观察。".to_string(),
        )
    };
    let a: Vec<f64> = rs
        .iter()
        .take(split)
        .filter_map(|r| r.accuracy)
        .rev()
        .take(10)
        .collect();
    let b: Vec<f64> = rs.iter().skip(split).filter_map(|r| r.accuracy).collect();
    let accuracy_change = if a.len() >= 5 && b.len() >= 5 {
        median(&a).zip(median(&b)).map(|(x, y)| (y - x) * 100.0)
    } else {
        None
    };
    let last30: Vec<&TrainingRun> = rs
        .iter()
        .filter(|r| r.timestamp >= now - 30 * 86400 && r.timestamp <= now)
        .copied()
        .collect();
    let plateau = if last30.len() >= 30
        && last30.last().unwrap().timestamp - last30[0].timestamp >= 21 * 86400
    {
        let mid = last30.len() / 2;
        let x: Vec<_> = last30[..mid].iter().map(|r| r.score).collect();
        let y: Vec<_> = last30[mid..].iter().map(|r| r.score).collect();
        change(median(&x), median(&y)).is_some_and(|d| d.abs() < 1.0)
            && cv(&y).is_some_and(|c| c < 10.0)
    } else {
        false
    };
    let m = median(&latest10);
    let deviations: Vec<_> = latest10
        .iter()
        .map(|x| (x - m.unwrap_or(0.0)).abs())
        .collect();
    let mad = median(&deviations).unwrap_or(0.0);
    let anomalies = if latest10.len() >= 10 {
        latest10
            .iter()
            .filter(|x| (**x - m.unwrap()).abs() > (mad * 3.0 * 1.4826).max(m.unwrap() * 0.15))
            .count()
    } else {
        0
    };
    ScenarioAnalysis {
        name: name.into(),
        count: n,
        pb,
        latest: rs.last().map(|r| r.score),
        median10: median(&latest10),
        previous_median: median(&past),
        median_change: delta,
        pb_change: pb_delta,
        variability,
        previous_variability: past_cv,
        last_trained: rs.last().map(|r| r.timestamp),
        accuracy_change,
        days7: window(&rs, now, 7),
        days30: window(&rs, now, 30),
        verdict: verdict.into(),
        explanation,
        plateau,
        anomalies,
        recent_count: recent.len(),
        previous_count: past.len(),
    }
}
pub fn all(runs: &[TrainingRun], now: i64) -> Vec<ScenarioAnalysis> {
    let names: std::collections::BTreeSet<_> = runs.iter().map(|r| r.scenario.as_str()).collect();
    names.into_iter().map(|n| scenario(n, runs, now)).collect()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub start: i64,
    pub end: i64,
    pub count: usize,
    pub decay: Vec<String>,
}
pub fn sessions(runs: &[TrainingRun], gap: u32) -> Vec<Session> {
    let mut rs: Vec<_> = runs.iter().collect();
    rs.sort_by_key(|r| r.timestamp);
    let mut groups: Vec<Vec<&TrainingRun>> = Vec::new();
    for r in rs {
        if groups
            .last()
            .is_none_or(|g| r.timestamp - g.last().unwrap().timestamp > gap as i64 * 60)
        {
            groups.push(Vec::new());
        }
        groups.last_mut().unwrap().push(r);
    }
    groups.into_iter().map(|g|{let names:std::collections::BTreeSet<_>=g.iter().map(|r|r.scenario.as_str()).collect();let mut decay=Vec::new();
        for name in names {let v:Vec<f64>=g.iter().filter(|r|r.scenario==name).map(|r|r.score).collect();if v.len()>=9 {let k=v.len()/3;let a=median(&v[..k]).unwrap();let b=median(&v[k..2*k]).unwrap();let c=median(&v[2*k..]).unwrap();if a>0.0 && b<a*0.97 && c<b*0.97 {decay.push(format!("{name}：同场景三段中位数持续下降，首末变化 {:.1}%。可能与疲劳有关，建议休息后复测。",(c-a)/a*100.0));}}}
        Session{start:g[0].timestamp,end:g.last().unwrap().timestamp,count:g.len(),decay}
    }).collect()
}
