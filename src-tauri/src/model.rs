use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingRun {
    pub run_id: String,
    pub scenario: String,
    pub timestamp: i64,
    pub timestamp_basis: String,
    pub score: f64,
    pub accuracy: Option<f64>,
    pub kills: Option<f64>,
    pub shots: Option<f64>,
    pub hits: Option<f64>,
    pub duration: Option<f64>,
    pub source_file: String,
    pub raw: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub stats_dir: String,
    pub auto_watch: bool,
    pub session_gap_minutes: u32,
    pub daily_minutes: u32,
    pub focus: String,
    pub target_rank: String,
    pub prioritize_weak: bool,
    pub maintain_strength: bool,
    pub ai_base_url: String,
    pub ai_model: String,
    pub ai_consent: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            stats_dir: String::new(),
            auto_watch: false,
            session_gap_minutes: 20,
            daily_minutes: 60,
            focus: String::new(),
            target_rank: String::new(),
            prioritize_weak: true,
            maintain_strength: true,
            ai_base_url: "https://api.openai.com/v1".into(),
            ai_model: String::new(),
            ai_consent: false,
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=180).contains(&self.session_gap_minutes) {
            return Err("会话间隔必须为 1–180 分钟".into());
        }
        if !(5..=240).contains(&self.daily_minutes) {
            return Err("每日时长必须为 5–240 分钟".into());
        }
        let url = reqwest::Url::parse(&self.ai_base_url)
            .map_err(|_| "API Base URL 不是有效 URL".to_string())?;
        if !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err("API Base URL 不得包含用户名、密码、查询参数或片段".into());
        }
        let local = matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"));
        if url.scheme() != "https" && !(url.scheme() == "http" && local) {
            return Err("外部 AI 地址必须使用 HTTPS；本机服务允许 HTTP".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ImportSummary {
    pub imported: usize,
    pub updated: usize,
    pub skipped: usize,
    pub failed: usize,
    pub deferred: usize,
    pub errors: Vec<String>,
    pub finished_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkConfig {
    pub name: String,
    pub version: String,
    pub difficulty: String,
    pub source: Option<String>,
    pub ranks: Vec<String>,
    pub scenarios: Vec<BenchmarkScenario>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkScenario {
    pub name: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub category: String,
    pub subcategory: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub thresholds: BTreeMap<String, f64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub id: i64,
    pub generated_at: i64,
    pub date: String,
    pub text: String,
    pub kind: String,
    pub evidence: serde_json::Value,
}
