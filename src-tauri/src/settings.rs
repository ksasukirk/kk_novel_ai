//! 全局设置
//! 代码路径: kk_novel_ai/src-tauri/src/settings.rs

use crate::error::AppResult;
use crate::paths::settings_path;
use serde::{Deserialize, Serialize};
use std::fs;

fn default_frequency_penalty() -> f32 {
    0.55
}

fn default_presence_penalty() -> f32 {
    0.25
}

fn default_llm_timeout_secs() -> u64 {
    600
}

fn default_true() -> bool {
    true
}

fn default_editor_font_family() -> String {
    // 前端按 id「heiti」解析为黑体栈；兼容直接存 CSS
    "heiti".into()
}

fn default_editor_font_size() -> u32 {
    16
}

/// 分析页列表每页条数
fn default_analytics_page_size() -> u32 {
    10
}

/// 每轮续写规定字数；0 表示未配置（加载时从 max_tokens 迁移）
fn default_writing_target_chars() -> u32 {
    0
}

fn default_api_provider() -> String {
    "local".into()
}

fn default_deepseek_pricing_tier() -> String {
    "auto".into()
}

fn default_true_for_deepseek_cache() -> bool {
    true
}

fn default_image_provider() -> String {
    "openai_compat".into()
}

fn default_image_size() -> String {
    "1024x1024".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentProject {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub opened_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub base_url: String,
    pub api_key: String,
    /// 写作模型（续写/润色/章纲）；兼容旧字段名 model
    pub model: String,
    #[serde(default)]
    pub analysis_model: String,
    #[serde(default)]
    pub analysis_temperature: Option<f32>,
    #[serde(default)]
    pub embedding_model: String,
    pub temperature: f32,
    /// 与规定字数对齐的 API 上限（保存时由 writing_target_chars 推导；运行时也以规定字数为准）
    pub max_tokens: u32,
    /// 每轮续写规定字数（目标篇幅）；最大生成量按此对齐
    #[serde(default = "default_writing_target_chars")]
    pub writing_target_chars: u32,
    /// OpenAI 兼容：抑制已出现 token 的复读（写作默认 0.55）
    #[serde(default = "default_frequency_penalty")]
    pub frequency_penalty: f32,
    /// OpenAI 兼容：鼓励新话题（写作默认 0.25）
    #[serde(default = "default_presence_penalty")]
    pub presence_penalty: f32,
    /// LLM 请求超时（秒）；大模型加载/生成宜 ≥ 600
    #[serde(default = "default_llm_timeout_secs")]
    pub llm_timeout_secs: u64,
    /// 续写检测到复读截断后，自动降参重试一次
    #[serde(default = "default_true")]
    pub writing_retry_on_loop: bool,
    /// 指定模型失败时回退到 settings.model
    #[serde(default = "default_true")]
    pub writing_model_fallback: bool,
    /// 关闭思考链（商汤 SenseNova 等推理模需要，否则 content 常为空）
    #[serde(default)]
    pub disable_thinking: Option<bool>,
    /// 长续写优先使用的强模型（如 deepseek-v4-pro）；空则尝试从 flash 推断
    #[serde(default)]
    pub writing_pro_model: String,
    /// 续写任务自动路由到 writing_pro_model（失败回退 model）
    #[serde(default = "default_true")]
    pub writing_route_pro_on_continue: bool,
    /// 续写写入后自动块级蒸馏记忆
    #[serde(default = "default_true")]
    pub writing_auto_digest: bool,
    /// 按纲续写时同步等待块蒸馏后再进下一节拍
    #[serde(default = "default_true")]
    pub writing_outline_run_sync_digest: bool,
    /// 生成写入后自动抽取新人物到本篇角色
    #[serde(default = "default_true")]
    pub writing_auto_cast: bool,
    /// 生成写入后自动增量同步总谱（故事线/时间线/关系/Canon）
    #[serde(default = "default_true")]
    pub writing_auto_story_sync: bool,
    /// 定稿时清洗「不是A是B」等否定对照口癖（默认开）
    #[serde(default = "default_true")]
    pub writing_strip_rhetoric: bool,
    /// 删除操作跳过确认弹窗（默认开）
    #[serde(default = "default_true")]
    pub skip_delete_confirm: bool,
    /// 写作区字体：预设 id（heiti/yahei/…）或 CSS font-family；默认黑体
    #[serde(default = "default_editor_font_family")]
    pub editor_font_family: String,
    /// 写作区字号（px），默认 16
    #[serde(default = "default_editor_font_size")]
    pub editor_font_size: u32,
    /// 分析页列表每页条数（作品 / 章节 / 生成记录），默认 10
    #[serde(default = "default_analytics_page_size")]
    pub analytics_page_size: u32,
    pub context_budget: u32,
    pub recent_window_chars: usize,
    /// 输入 token 单价（元 / 百万 tokens）；本地默认 0；DeepSeek 时作「缓存未命中」单价
    #[serde(default)]
    pub price_input_per_1m: f64,
    /// 输出 token 单价（元 / 百万 tokens）；本地默认 0
    #[serde(default)]
    pub price_output_per_1m: f64,
    /// API 接入预设：local | deepseek_flash | deepseek_pro | custom
    #[serde(default = "default_api_provider")]
    pub api_provider: String,
    /// DeepSeek 计费时段：idle（空闲半价）| peak（高峰）| auto（按北京时间推断）
    #[serde(default = "default_deepseek_pricing_tier")]
    pub deepseek_pricing_tier: String,
    /// 缓存命中输入单价（元 / 百万 tokens）；0 则回退 price_input 的 1/30（DeepSeek flash 空闲比）
    #[serde(default)]
    pub price_cache_hit_per_1m: f64,
    /// 续写 prompt 把易变字段（节拍状态/方向锚点等）放到末尾，利于 DeepSeek 前缀缓存
    #[serde(default = "default_true_for_deepseek_cache")]
    pub writing_cache_friendly_prompt: bool,
    /// 图像 API：openai_compat（Comfy 第二期）
    #[serde(default = "default_image_provider")]
    pub image_provider: String,
    #[serde(default)]
    pub image_base_url: String,
    #[serde(default)]
    pub image_api_key: String,
    #[serde(default)]
    pub image_model: String,
    #[serde(default = "default_image_size")]
    pub image_size: String,
    #[serde(default)]
    pub last_project_path: Option<String>,
    /// 最近作品列表（作品页网格）
    #[serde(default)]
    pub recent_projects: Vec<RecentProject>,
    /// 最近知识库（知识库首页）
    #[serde(default)]
    pub recent_knowledge_bases: Vec<RecentProject>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            base_url: default_base_url_for_platform(),
            api_key: if crate::paths::is_mobile() {
                String::new()
            } else {
                "lm-studio".into()
            },
            model: String::new(),
            analysis_model: String::new(),
            analysis_temperature: None,
            embedding_model: String::new(),
            temperature: 0.8,
            writing_target_chars: 1800,
            max_tokens: ((1800_f64) * 1.15).ceil() as u32,
            frequency_penalty: default_frequency_penalty(),
            presence_penalty: default_presence_penalty(),
            llm_timeout_secs: default_llm_timeout_secs(),
            writing_retry_on_loop: true,
            writing_model_fallback: true,
            disable_thinking: None,
            writing_pro_model: String::new(),
            writing_route_pro_on_continue: true,
            writing_auto_digest: true,
            writing_outline_run_sync_digest: true,
            writing_auto_cast: true,
            writing_auto_story_sync: true,
            writing_strip_rhetoric: true,
            skip_delete_confirm: true,
            editor_font_family: default_editor_font_family(),
            editor_font_size: default_editor_font_size(),
            analytics_page_size: default_analytics_page_size(),
            context_budget: 12000,
            recent_window_chars: 3000,
            price_input_per_1m: 0.0,
            price_output_per_1m: 0.0,
            api_provider: default_api_provider(),
            deepseek_pricing_tier: default_deepseek_pricing_tier(),
            price_cache_hit_per_1m: 0.0,
            writing_cache_friendly_prompt: true,
            image_provider: default_image_provider(),
            image_base_url: String::new(),
            image_api_key: String::new(),
            image_model: String::new(),
            image_size: default_image_size(),
            last_project_path: None,
            recent_projects: vec![],
            recent_knowledge_bases: vec![],
        }
    }
}

impl AppSettings {
    /// 写作类任务用的模型 ID
    pub fn writing_model(&self) -> &str {
        &self.model
    }

    /// 每轮续写规定字数（正文目标）
    pub fn resolve_writing_target_chars(&self) -> u32 {
        if self.writing_target_chars >= 200 {
            self.writing_target_chars
        } else if self.max_tokens >= 200 {
            // 旧配置未写规定字数：把原 max_tokens 当作规定字数
            self.max_tokens
        } else {
            1800
        }
    }

    /// API max_tokens：须能写出「至少规定字数且可超出」；按约 1.8× 规定字数留预算
    pub fn resolve_writing_max_tokens(&self) -> u32 {
        let chars = self.resolve_writing_target_chars();
        let mt = ((chars as f64) * 1.8).ceil() as u32;
        mt.max(256).min(32768)
    }

    /// 保存前：让 max_tokens 与规定字数对齐
    pub fn sync_max_tokens_to_target(&mut self) {
        if self.writing_target_chars < 200 {
            self.writing_target_chars = if self.max_tokens >= 200 {
                self.max_tokens
            } else {
                1800
            };
        }
        self.max_tokens = self.resolve_writing_max_tokens();
    }

    /// 解析强模型 id：显式配置 > DeepSeek flash 时默认 pro
    pub fn resolve_pro_model(&self) -> Option<String> {
        let p = self.writing_pro_model.trim();
        if !p.is_empty() {
            return Some(p.to_string());
        }
        let m = self.model.to_lowercase();
        let u = self.base_url.to_lowercase();
        if u.contains("deepseek.com") && m.contains("flash") {
            return Some("deepseek-v4-pro".into());
        }
        None
    }

    /// 按任务路由写作模型：长文 continue 可走 pro
    pub fn resolve_writing_model_for_task(&self, task: &str, _chapter_chars: usize) -> String {
        let base = self.writing_model().to_string();
        if task != "continue" || !self.writing_route_pro_on_continue {
            return base;
        }
        match self.resolve_pro_model() {
            Some(pro) if pro != base => pro,
            _ => base,
        }
    }

    /// 分析类（摘要/一致性）模型；空则回退写作模型
    pub fn resolve_analysis_model(&self) -> &str {
        if self.analysis_model.trim().is_empty() {
            &self.model
        } else {
            &self.analysis_model
        }
    }

    pub fn resolve_analysis_temperature(&self) -> f32 {
        self.analysis_temperature.unwrap_or(0.3)
    }

    pub fn resolve_disable_thinking(&self) -> bool {
        if let Some(v) = self.disable_thinking {
            return v;
        }
        // 商汤 / DeepSeek 等推理模：不关思考链时，长写作 prompt 常把 max_tokens 耗在 reasoning 上，content 为空
        let u = self.base_url.to_lowercase();
        u.contains("sensenova.cn") || u.contains("deepseek.com")
    }

    pub fn resolve_embedding_model(&self) -> Option<&str> {
        let s = self.embedding_model.trim();
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    }

    pub fn is_deepseek(&self) -> bool {
        let u = self.base_url.to_lowercase();
        u.contains("deepseek.com")
            || matches!(
                self.api_provider.as_str(),
                "deepseek_flash" | "deepseek_pro"
            )
    }

    /// 北京时间是否处于 DeepSeek 高峰（周一至周五 9-12、14-18）
    pub fn deepseek_is_peak_now() -> bool {
        use chrono::{Datelike, Timelike, Utc};
        let bj = Utc::now() + chrono::Duration::hours(8);
        let wd = bj.weekday().num_days_from_monday();
        if wd >= 5 {
            return false;
        }
        let h = bj.hour();
        (9..12).contains(&h) || (14..18).contains(&h)
    }

    pub fn resolve_deepseek_peak(&self) -> bool {
        match self.deepseek_pricing_tier.trim().to_lowercase().as_str() {
            "peak" | "high" => true,
            "idle" | "off_peak" | "offpeak" => false,
            _ => Self::deepseek_is_peak_now(),
        }
    }

    /// DeepSeek 官方 flash / pro 空闲与高峰单价（元/百万 tokens）
    pub fn deepseek_official_prices(model: &str, peak: bool) -> (f64, f64, f64) {
        let pro = model.contains("pro");
        if peak {
            if pro {
                (0.30, 9.0, 27.0)
            } else {
                (0.10, 3.0, 9.0)
            }
        } else if pro {
            (0.15, 4.5, 13.5)
        } else {
            (0.05, 1.5, 4.5)
        }
    }

    /// 应用 DeepSeek 预设到当前设置（保留 api_key）
    pub fn apply_deepseek_preset(&mut self, variant: &str) {
        let peak = self.resolve_deepseek_peak();
        let (hit, miss, out) = match variant {
            "deepseek_pro" | "pro" => {
                self.model = "deepseek-v4-pro".into();
                self.analysis_model = "deepseek-v4-flash".into();
                self.writing_pro_model = "deepseek-v4-pro".into();
                Self::deepseek_official_prices("pro", peak)
            }
            _ => {
                self.model = "deepseek-v4-flash".into();
                self.analysis_model = "deepseek-v4-flash".into();
                self.writing_pro_model = String::new();
                Self::deepseek_official_prices("flash", peak)
            }
        };
        self.api_provider = if variant == "deepseek_pro" || variant == "pro" {
            "deepseek_pro".into()
        } else {
            "deepseek_flash".into()
        };
        self.base_url = "https://api.deepseek.com".into();
        self.disable_thinking = Some(true);
        self.writing_route_pro_on_continue = variant == "deepseek_pro" || variant == "pro";
        self.writing_cache_friendly_prompt = true;
        self.price_cache_hit_per_1m = hit;
        self.price_input_per_1m = miss;
        self.price_output_per_1m = out;
        self.llm_timeout_secs = 600;
        self.context_budget = 24000;
        self.recent_window_chars = 2500;
    }

    /// 按当前时段刷新 DeepSeek 官方单价（不改模型与 base_url）
    pub fn refresh_deepseek_prices(&mut self) {
        if !self.is_deepseek() {
            return;
        }
        let peak = self.resolve_deepseek_peak();
        let model_key = if self.model.to_lowercase().contains("pro") {
            "pro"
        } else {
            "flash"
        };
        let (hit, miss, out) = Self::deepseek_official_prices(model_key, peak);
        self.price_cache_hit_per_1m = hit;
        self.price_input_per_1m = miss;
        self.price_output_per_1m = out;
    }

    pub fn resolve_prices_for_model(&self, model_used: &str) -> (f64, f64, f64) {
        if self.is_deepseek() {
            let peak = self.resolve_deepseek_peak();
            let m = if model_used.trim().is_empty() {
                self.model.as_str()
            } else {
                model_used
            };
            let pro = m.to_lowercase().contains("pro");
            return Self::deepseek_official_prices(if pro { "pro" } else { "flash" }, peak);
        }
        let hit = if self.price_cache_hit_per_1m > 0.0 {
            self.price_cache_hit_per_1m
        } else {
            self.price_input_per_1m.max(0.0)
        };
        (
            hit,
            self.price_input_per_1m.max(0.0),
            self.price_output_per_1m.max(0.0),
        )
    }

    /// 高峰时段生成前提示文案；非 DeepSeek 或非高峰返回 None
    pub fn deepseek_peak_notice(&self) -> Option<String> {
        if !self.is_deepseek() || !self.resolve_deepseek_peak() {
            return None;
        }
        Some(
            "当前为 DeepSeek 高峰时段（周一至周五 9:00–12:00、14:00–18:00 北京时间），API 单价为空闲时段的 2 倍；大批量生成建议改到晚间或周末".into(),
        )
    }

    /// 加载/保存时把官方单价写入设置字段（便于 UI 展示）
    pub fn sync_deepseek_price_fields(&mut self) {
        if !self.is_deepseek() {
            return;
        }
        if self.deepseek_pricing_tier.trim().is_empty() {
            self.deepseek_pricing_tier = "auto".into();
        }
        self.refresh_deepseek_prices();
    }

    pub fn resolve_cache_hit_price(&self, model_used: &str) -> f64 {
        if self.is_deepseek() {
            return self.resolve_prices_for_model(model_used).0;
        }
        if self.price_cache_hit_per_1m > 0.0 {
            return self.price_cache_hit_per_1m;
        }
        self.price_input_per_1m.max(0.0)
    }

    pub fn resolve_cache_miss_price(&self, model_used: &str) -> f64 {
        if self.is_deepseek() {
            return self.resolve_prices_for_model(model_used).1;
        }
        self.price_input_per_1m.max(0.0)
    }

    pub fn resolve_output_price(&self, model_used: &str) -> f64 {
        if self.is_deepseek() {
            return self.resolve_prices_for_model(model_used).2;
        }
        self.price_output_per_1m.max(0.0)
    }

    pub fn normalize_base_url(url: &str) -> String {
        let u = url.trim().trim_end_matches('/');
        if u.is_empty() {
            return u.to_string();
        }
        // DeepSeek 官方不含 /v1；误填时自动纠正
        if u.contains("deepseek.com") && u.ends_with("/v1") {
            return u.trim_end_matches("/v1").to_string();
        }
        u.to_string()
    }

    pub fn touch_recent_project(&mut self, path: &str, title: &str) {
        let path = path.to_string();
        self.recent_projects.retain(|p| p.path != path);
        self.recent_projects.insert(
            0,
            RecentProject {
                path: path.clone(),
                title: if title.is_empty() {
                    "未命名小说".into()
                } else {
                    title.to_string()
                },
                opened_at: chrono::Utc::now().to_rfc3339(),
            },
        );
        // 不截断：作品页需展示全部已登记项目
        self.last_project_path = Some(path);
    }

    /// 作品目录重命名后，把最近列表里的旧路径换成新路径
    pub fn replace_recent_project_path(&mut self, old_path: &str, new_path: &str, title: &str) {
        let new_path = new_path.to_string();
        let title = if title.is_empty() {
            "未命名小说".to_string()
        } else {
            title.to_string()
        };
        let mut item = None;
        if let Some(i) = self.recent_projects.iter().position(|p| p.path == old_path) {
            item = Some(self.recent_projects.remove(i));
        }
        let entry = item.unwrap_or(RecentProject {
            path: new_path.clone(),
            title: title.clone(),
            opened_at: chrono::Utc::now().to_rfc3339(),
        });
        let mut entry = entry;
        entry.path = new_path.clone();
        entry.title = title;
        entry.opened_at = chrono::Utc::now().to_rfc3339();
        self.recent_projects.insert(0, entry);
        if self.last_project_path.as_deref() == Some(old_path) {
            self.last_project_path = Some(new_path);
        }
    }

    pub fn touch_recent_knowledge_base(&mut self, path: &str, title: &str) {
        let path = path.to_string();
        self.recent_knowledge_bases.retain(|p| p.path != path);
        self.recent_knowledge_bases.insert(
            0,
            RecentProject {
                path: path.clone(),
                title: if title.is_empty() {
                    "未命名知识库".into()
                } else {
                    title.to_string()
                },
                opened_at: chrono::Utc::now().to_rfc3339(),
            },
        );
        if self.recent_knowledge_bases.len() > 48 {
            self.recent_knowledge_bases.truncate(48);
        }
        // 知识库不进写作最近列表
        self.recent_projects.retain(|p| p.path != path);
    }

    pub fn remove_recent_project(&mut self, path: &str) {
        self.recent_projects.retain(|p| p.path != path);
        self.recent_knowledge_bases.retain(|p| p.path != path);
        if self.last_project_path.as_deref() == Some(path) {
            self.last_project_path = self.recent_projects.first().map(|p| p.path.clone());
        }
    }
}

pub fn load_settings() -> AppResult<AppSettings> {
    let path = settings_path()?;
    if !path.exists() {
        let mut defaults = AppSettings::default();
        defaults.sync_max_tokens_to_target();
        save_settings(&defaults)?;
        return Ok(defaults);
    }
    let text = fs::read_to_string(&path)?;
    let mut settings: AppSettings = serde_json::from_str(&text)?;
    // 兼容：仅有 last_project_path 时补进 recent
    if settings.recent_projects.is_empty() {
        if let Some(p) = settings.last_project_path.clone() {
            settings.recent_projects.push(RecentProject {
                path: p,
                title: "最近作品".into(),
                opened_at: String::new(),
            });
        }
    }
    // 旧配置无 writing_target_chars 时，用 max_tokens 当作规定字数并回写对齐
    let before = (settings.writing_target_chars, settings.max_tokens);
    settings.sync_max_tokens_to_target();
    settings.base_url = AppSettings::normalize_base_url(&settings.base_url);
    let price_before = (
        settings.price_cache_hit_per_1m,
        settings.price_input_per_1m,
        settings.price_output_per_1m,
    );
    settings.sync_deepseek_price_fields();
    let need_save = (settings.writing_target_chars, settings.max_tokens) != before
        || (settings.is_deepseek()
            && (
                settings.price_cache_hit_per_1m,
                settings.price_input_per_1m,
                settings.price_output_per_1m,
            ) != price_before);
    if need_save {
        let _ = save_settings(&settings);
    }
    Ok(settings)
}

pub fn save_settings(settings: &AppSettings) -> AppResult<()> {
    validate_for_platform(settings)?;
    let path = settings_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut synced = settings.clone();
    synced.base_url = AppSettings::normalize_base_url(&synced.base_url);
    synced.sync_deepseek_price_fields();
    synced.sync_max_tokens_to_target();
    let text = serde_json::to_string_pretty(&synced)?;
    fs::write(path, text)?;
    Ok(())
}

/// 手机端禁止把 localhost/127.0.0.1 当作可用 LLM 地址
pub fn validate_for_platform(settings: &AppSettings) -> AppResult<()> {
    if !crate::paths::is_mobile() {
        return Ok(());
    }
    let u = settings.base_url.trim().to_lowercase();
    if u.is_empty() {
        return Err(crate::error::AppError::msg(
            "请填写可访问的 API Base URL（局域网或公网 OpenAI 兼容地址）",
        ));
    }
    if u.contains("127.0.0.1") || u.contains("localhost") || u.contains("[::1]") {
        return Err(crate::error::AppError::msg(
            "手机端不能使用 localhost / 127.0.0.1（指向手机自身）。请填写电脑局域网 IP 或公网 HTTPS 地址，例如 http://192.168.1.8:1234/v1",
        ));
    }
    Ok(())
}

/// 移动端默认 Base URL：留空提示，避免误连本机
pub fn default_base_url_for_platform() -> String {
    if crate::paths::is_mobile() {
        String::new()
    } else {
        "http://127.0.0.1:1234/v1".into()
    }
}
