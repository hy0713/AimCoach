use crate::{analysis, model::*};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
fn norm(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}
pub fn validate(b: &BenchmarkConfig) -> Result<(), String> {
    if b.name.trim().is_empty() || b.version.trim().is_empty() || b.difficulty.trim().is_empty() {
        return Err("Benchmark 名称、版本和难度不能为空".into());
    }
    if b.ranks.is_empty() || b.scenarios.is_empty() {
        return Err("必须提供等级列表和至少一个真实场景；不接受空模板作为有效 Benchmark".into());
    }
    if b.ranks.len() > 30 || b.scenarios.len() > 1000 {
        return Err("最多 30 个等级、1000 个场景".into());
    }
    let ranks: BTreeSet<_> = b.ranks.iter().map(|s| norm(s)).collect();
    if ranks.len() != b.ranks.len() || ranks.contains("") {
        return Err("等级名称为空或重复".into());
    }
    let mut names = BTreeSet::new();
    for s in &b.scenarios {
        if s.name.trim().is_empty()
            || s.category.trim().is_empty()
            || s.subcategory.trim().is_empty()
        {
            return Err("场景名称、大类和子类不能为空".into());
        }
        for name in std::iter::once(&s.name).chain(s.aliases.iter()) {
            if norm(name).is_empty() || !names.insert(norm(name)) {
                return Err(format!("场景/别名重复：{name}"));
            }
        }
        if s.thresholds.len() != b.ranks.len() {
            return Err(format!("{} 的门槛必须完整对应所有等级", s.name));
        }
        let mut previous = 0.0;
        for rank in &b.ranks {
            let v = *s
                .thresholds
                .get(rank)
                .ok_or_else(|| format!("{} 缺少 {rank} 门槛", s.name))?;
            if !v.is_finite() || v <= previous {
                return Err(format!("{} 的门槛必须为有限正数且严格递增", s.name));
            }
            previous = v;
        }
    }
    Ok(())
}
pub fn matches(s: &BenchmarkScenario, name: &str) -> bool {
    norm(&s.name) == norm(name) || s.aliases.iter().any(|a| norm(a) == norm(name))
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Level {
    pub rank: Option<String>,
    pub rank_index: i32,
    pub progress: f64,
    pub normalized: f64,
    pub next_rank: Option<String>,
    pub next_score: Option<f64>,
}
pub fn level(score: f64, s: &BenchmarkScenario, ranks: &[String]) -> Level {
    let mut index = -1;
    for (i, r) in ranks.iter().enumerate() {
        if score >= s.thresholds[r] {
            index = i as i32;
        } else {
            break;
        }
    }
    let next = (index + 1) as usize;
    let lower = if index < 0 {
        0.0
    } else {
        s.thresholds[&ranks[index as usize]]
    };
    let (progress, next_rank, next_score) = if next < ranks.len() {
        let high = s.thresholds[&ranks[next]];
        (
            ((score - lower) / (high - lower)).clamp(0.0, 1.0),
            Some(ranks[next].clone()),
            Some(high),
        )
    } else {
        (1.0, None, None)
    };
    let normalized = if next >= ranks.len() {
        ranks.len() as f64
    } else {
        index as f64 + 1.0 + progress
    };
    Level {
        rank: if index < 0 {
            None
        } else {
            Some(ranks[index as usize].clone())
        },
        rank_index: index,
        progress,
        normalized,
        next_rank,
        next_score,
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkRow {
    pub name: String,
    pub category: String,
    pub subcategory: String,
    pub matched_names: Vec<String>,
    pub count: usize,
    pub pb: Option<f64>,
    pub median10: Option<f64>,
    pub best: Option<Level>,
    pub stable: Option<Level>,
    pub target_score: f64,
    pub target_met: bool,
    pub recent_change: Option<f64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillGroup {
    pub name: String,
    pub mean: Option<f64>,
    pub best: Option<f64>,
    pub lowest: Option<f64>,
    pub relative: Option<f64>,
    pub change: Option<f64>,
    pub runs: usize,
    pub covered: usize,
    pub total: usize,
    pub confidence: String,
    pub weakest_subcategory: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Complete {
    pub rank: String,
    pub reached: usize,
    pub total: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Upgrade {
    pub scenario: String,
    pub rank: String,
    pub timestamp: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkView {
    pub config: BenchmarkConfig,
    pub rows: Vec<BenchmarkRow>,
    pub categories: Vec<SkillGroup>,
    pub subcategories: Vec<SkillGroup>,
    pub complete: Vec<Complete>,
    pub rank_counts: BTreeMap<String, usize>,
    pub upgrades: Vec<Upgrade>,
    pub unmatched: Vec<String>,
    pub target: String,
}
fn groups(rows: &[BenchmarkRow], sub: bool) -> Vec<SkillGroup> {
    let names: BTreeSet<String> = rows
        .iter()
        .map(|r| {
            if sub {
                format!("{} / {}", r.category, r.subcategory)
            } else {
                r.category.clone()
            }
        })
        .collect();
    let mut result: Vec<_> = names
        .into_iter()
        .map(|name| {
            let members: Vec<_> = rows
                .iter()
                .filter(|r| {
                    if sub {
                        format!("{} / {}", r.category, r.subcategory) == name
                    } else {
                        r.category == name
                    }
                })
                .collect();
            let v: Vec<_> = members
                .iter()
                .filter_map(|r| r.stable.as_ref().map(|l| l.normalized))
                .collect();
            let changes: Vec<_> = members.iter().filter_map(|r| r.recent_change).collect();
            let runs = members.iter().map(|r| r.count).sum();
            let mut subs: BTreeMap<String, Vec<f64>> = BTreeMap::new();
            for r in &members {
                if let Some(l) = &r.stable {
                    subs.entry(r.subcategory.clone())
                        .or_default()
                        .push(l.normalized);
                }
            }
            let weakest = subs
                .into_iter()
                .map(|(n, v)| (n, v.iter().sum::<f64>() / v.len() as f64))
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|v| v.0);
            SkillGroup {
                name,
                mean: if v.is_empty() {
                    None
                } else {
                    Some(v.iter().sum::<f64>() / v.len() as f64)
                },
                best: members
                    .iter()
                    .filter_map(|r| r.best.as_ref().map(|l| l.normalized))
                    .reduce(f64::max),
                lowest: v.iter().copied().reduce(f64::min),
                relative: None,
                change: if changes.is_empty() {
                    None
                } else {
                    Some(changes.iter().sum::<f64>() / changes.len() as f64)
                },
                runs,
                covered: v.len(),
                total: members.len(),
                confidence: if v.is_empty() {
                    "无数据"
                } else if v.len() < members.len() || members.iter().any(|r| r.count < 10) {
                    "有限：缺场景或每场景不足 10 次"
                } else {
                    "较充分：全部覆盖且每场景至少 10 次"
                }
                .into(),
                weakest_subcategory: weakest,
            }
        })
        .collect();
    let values: Vec<_> = result.iter().filter_map(|g| g.mean).collect();
    if !values.is_empty() {
        let overall = values.iter().sum::<f64>() / values.len() as f64;
        for g in &mut result {
            g.relative = g.mean.map(|v| v - overall);
        }
    }
    result
}
pub fn view(b: &BenchmarkConfig, runs: &[TrainingRun], target: &str, now: i64) -> BenchmarkView {
    let target = if b.ranks.iter().any(|r| r == target) {
        target.to_owned()
    } else {
        b.ranks[0].clone()
    };
    let mut upgrades = Vec::new();
    let rows: Vec<_> = b
        .scenarios
        .iter()
        .map(|s| {
            let mut rs: Vec<_> = runs
                .iter()
                .filter(|r| matches(s, &r.scenario))
                .cloned()
                .collect();
            rs.sort_by_key(|r| r.timestamp);
            let matched_names: BTreeSet<_> = rs.iter().map(|r| r.scenario.clone()).collect();
            let n = rs.len();
            let pb = rs.iter().map(|r| r.score).reduce(f64::max);
            let med = analysis::median(
                &rs.iter()
                    .skip(n.saturating_sub(10))
                    .map(|r| r.score)
                    .collect::<Vec<_>>(),
            );
            let mut reached = -1;
            for r in &rs {
                let l = level(r.score, s, &b.ranks);
                if l.rank_index > reached {
                    for k in reached + 1..=l.rank_index {
                        upgrades.push(Upgrade {
                            scenario: s.name.clone(),
                            rank: b.ranks[k as usize].clone(),
                            timestamp: r.timestamp,
                        });
                    }
                    reached = l.rank_index;
                }
            }
            let canonical: Vec<_> = rs
                .iter()
                .map(|r| TrainingRun {
                    scenario: s.name.clone(),
                    ..r.clone()
                })
                .collect();
            let a = analysis::scenario(&s.name, &canonical, now);
            BenchmarkRow {
                name: s.name.clone(),
                category: s.category.clone(),
                subcategory: s.subcategory.clone(),
                matched_names: matched_names.into_iter().collect(),
                count: n,
                pb,
                median10: med,
                best: pb.map(|v| level(v, s, &b.ranks)),
                stable: med.map(|v| level(v, s, &b.ranks)),
                target_score: s.thresholds[&target],
                target_met: pb.is_some_and(|p| p >= s.thresholds[&target]),
                recent_change: if a.recent_count >= 5 {
                    a.median_change
                } else {
                    None
                },
            }
        })
        .collect();
    let complete = b
        .ranks
        .iter()
        .map(|r| Complete {
            rank: r.clone(),
            reached: rows
                .iter()
                .filter(|row| {
                    row.pb.is_some_and(|p| {
                        p >= b
                            .scenarios
                            .iter()
                            .find(|s| s.name == row.name)
                            .unwrap()
                            .thresholds[r]
                    })
                })
                .count(),
            total: rows.len(),
        })
        .collect();
    let mut rank_counts = BTreeMap::new();
    for r in &rows {
        let label = r
            .best
            .as_ref()
            .and_then(|l| l.rank.clone())
            .unwrap_or_else(|| {
                if r.pb.is_none() {
                    "未训练".into()
                } else {
                    "未达首级".into()
                }
            });
        *rank_counts.entry(label).or_default() += 1;
    }
    let categories = groups(&rows, false);
    let subcategories = groups(&rows, true);
    upgrades.sort_by_key(|u| std::cmp::Reverse(u.timestamp));
    upgrades.truncate(30);
    let names: BTreeSet<_> = runs.iter().map(|r| r.scenario.clone()).collect();
    let unmatched = names
        .into_iter()
        .filter(|n| !b.scenarios.iter().any(|s| matches(s, n)))
        .collect();
    BenchmarkView {
        config: b.clone(),
        rows,
        categories,
        subcategories,
        complete,
        rank_counts,
        upgrades,
        unmatched,
        target,
    }
}
