//! 后端面向用户的错误文案 i18n
//! 代码路径: kk_novel_ai/src-tauri/src/i18n.rs

use serde_json::Value;
use std::cell::Cell;
use std::sync::OnceLock;

const ZH_CN_JSON: &str = include_str!("../locales/zh-CN.json");
const EN_JSON: &str = include_str!("../locales/en.json");
const JA_JSON: &str = include_str!("../locales/ja.json");

static ZH_CN: OnceLock<Value> = OnceLock::new();
static EN: OnceLock<Value> = OnceLock::new();
static JA: OnceLock<Value> = OnceLock::new();

fn parse_catalog(raw: &'static str) -> Value {
    serde_json::from_str(raw).unwrap_or(Value::Null)
}

fn catalog(locale: &str) -> &'static Value {
    match locale {
        "en" => EN.get_or_init(|| parse_catalog(EN_JSON)),
        "ja" => JA.get_or_init(|| parse_catalog(JA_JSON)),
        _ => ZH_CN.get_or_init(|| parse_catalog(ZH_CN_JSON)),
    }
}

fn lookup(root: &Value, key: &str) -> Option<String> {
    let mut cur = root;
    for part in key.split('.') {
        cur = cur.get(part)?;
    }
    cur.as_str().map(|s| s.to_string())
}

fn current_ui_locale() -> String {
    thread_local! {
        static IN_LOAD: Cell<bool> = Cell::new(false);
    }
    IN_LOAD.with(|flag| {
        if flag.get() {
            return "zh-CN".to_string();
        }
        flag.set(true);
        let loc = crate::settings::load_settings()
            .map(|s| crate::settings::AppSettings::normalize_locale_code(&s.ui_locale))
            .unwrap_or_else(|_| "zh-CN".into());
        flag.set(false);
        loc
    })
}

/// 按当前设置 `ui_locale` 取文案；缺词回退 zh-CN，再回退 key 本身
pub fn t(key: &str) -> String {
    let locale = current_ui_locale();
    if let Some(s) = lookup(catalog(&locale), key) {
        return s;
    }
    if locale != "zh-CN" {
        if let Some(s) = lookup(catalog("zh-CN"), key) {
            return s;
        }
    }
    key.to_string()
}

/// `{name}` 占位替换
pub fn t_fmt(key: &str, args: &[(&str, &str)]) -> String {
    let mut s = t(key);
    for (name, value) in args {
        s = s.replace(&format!("{{{name}}}"), value);
    }
    s
}
