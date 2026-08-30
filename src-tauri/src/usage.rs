//! Token / 费用累计账本
//! 代码路径: kk_novel_ai/src-tauri/src/usage.rs

use crate::error::AppResult;
use crate::genlog::GenLogEntry;
use crate::llm::TokenUsage;
use crate::paths::usage_ledger_path;
use crate::settings::AppSettings;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelBucket {
    #[serde(default)]
    pub prompt_tokens: u64,
    #[serde(default)]
    pub completion_tokens: u64,
    #[serde(default)]
    pub cost_cny: f64,
    #[serde(default)]
    pub calls: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UsageLedger {
    #[serde(default)]
    pub total_prompt_tokens: u64,
    #[serde(default)]
    pub total_completion_tokens: u64,
    #[serde(default)]
    pub total_cost_cny: f64,
    #[serde(default)]
    pub total_calls: u64,
    #[serde(default)]
    pub by_model: HashMap<String, ModelBucket>,
    #[serde(default)]
    pub by_project: HashMap<String, ModelBucket>,
}

pub fn calc_cost_cny(usage: &TokenUsage, settings: &AppSettings) -> f64 {
    let pin = settings.price_input_per_1m.max(0.0);
    let pout = settings.price_output_per_1m.max(0.0);
    (usage.prompt_tokens as f64 / 1_000_000.0) * pin
        + (usage.completion_tokens as f64 / 1_000_000.0) * pout
}

pub fn load_ledger() -> AppResult<UsageLedger> {
    let path = usage_ledger_path()?;
    if !path.exists() {
        return Ok(UsageLedger::default());
    }
    let text = fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&text).unwrap_or_default())
}

pub fn save_ledger(ledger: &UsageLedger) -> AppResult<()> {
    let path = usage_ledger_path()?;
    let text = serde_json::to_string_pretty(ledger)?;
    fs::write(path, text)?;
    Ok(())
}

fn bump(bucket: &mut ModelBucket, usage: &TokenUsage, cost: f64) {
    bucket.prompt_tokens += usage.prompt_tokens as u64;
    bucket.completion_tokens += usage.completion_tokens as u64;
    bucket.cost_cny += cost;
    bucket.calls += 1;
}

pub fn record_entry(entry: &GenLogEntry) -> AppResult<()> {
    let mut ledger = load_ledger()?;
    let usage = entry.usage.clone().unwrap_or_default();
    let cost = entry.cost_cny;
    ledger.total_prompt_tokens += usage.prompt_tokens as u64;
    ledger.total_completion_tokens += usage.completion_tokens as u64;
    ledger.total_cost_cny += cost;
    ledger.total_calls += 1;

    let model_key = if entry.model_used.trim().is_empty() {
        "(unknown)".to_string()
    } else {
        entry.model_used.clone()
    };
    bump(ledger.by_model.entry(model_key).or_default(), &usage, cost);

    let proj = if entry.project_root.trim().is_empty() {
        "(none)".to_string()
    } else {
        entry.project_root.clone()
    };
    bump(ledger.by_project.entry(proj).or_default(), &usage, cost);

    save_ledger(&ledger)
}

pub fn summary_json(project_root: Option<&str>) -> AppResult<serde_json::Value> {
    let ledger = load_ledger()?;
    let project = project_root
        .filter(|s| !s.is_empty())
        .and_then(|p| ledger.by_project.get(p).cloned());
    Ok(json!({
        "ok": true,
        "global": {
            "prompt_tokens": ledger.total_prompt_tokens,
            "completion_tokens": ledger.total_completion_tokens,
            "total_tokens": ledger.total_prompt_tokens + ledger.total_completion_tokens,
            "cost_cny": ledger.total_cost_cny,
            "calls": ledger.total_calls,
        },
        "project": project,
        "by_model": ledger.by_model,
    }))
}
