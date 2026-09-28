//! 情节/喜好规范文风标签（与前端 tropeStyles.js 对齐）
//! 代码路径: kk_novel_ai/src-tauri/src/project/trope_styles.rs

pub const CANON_STYLES: &[&str] = &[
    "含蓄", "露骨", "直言不讳", "肮脏", "细腻", "粗口", "克制", "淫秽",
];

const ALIASES: &[(&str, &str)] = &[
    ("含蓄", "含蓄"),
    ("委婉", "含蓄"),
    ("暗示", "含蓄"),
    ("隐晦", "含蓄"),
    ("露骨", "露骨"),
    ("直白露骨", "露骨"),
    ("explicit", "露骨"),
    ("直言不讳", "直言不讳"),
    ("直白", "直言不讳"),
    ("不掩饰", "直言不讳"),
    ("blunt", "直言不讳"),
    ("肮脏", "肮脏"),
    ("dirty", "肮脏"),
    ("龌龊", "肮脏"),
    ("下贱", "肮脏"),
    ("细腻", "细腻"),
    ("细致", "细腻"),
    ("感官细", "细腻"),
    ("粗口", "粗口"),
    ("脏话", "粗口"),
    ("脏话连篇", "粗口"),
    ("克制", "克制"),
    ("收敛", "克制"),
    ("节制", "克制"),
    ("淫秽", "淫秽"),
    ("秽亵", "淫秽"),
    ("obscene", "淫秽"),
];

fn is_canon(s: &str) -> bool {
    CANON_STYLES.iter().any(|c| *c == s)
}

/// 单段文本收到规范文风名；对不上返回空串
pub fn normalize_style(raw: &str) -> String {
    let s = raw.trim();
    if s.is_empty() {
        return String::new();
    }
    if is_canon(s) {
        return s.to_string();
    }
    let hay: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    let hay_lower = hay.to_ascii_lowercase();
    let mut best = "";
    let mut best_len = 0usize;
    for (alias, canon) in ALIASES {
        let alias_len = alias.chars().count();
        if alias_len < best_len {
            continue;
        }
        let hit = if alias.is_ascii() {
            hay_lower.contains(&alias.to_ascii_lowercase())
        } else {
            hay.contains(alias)
        };
        if hit {
            best = canon;
            best_len = alias_len;
        }
    }
    best.to_string()
}

fn split_raw(raw: &str) -> Vec<String> {
    raw.split(|c: char| matches!(c, ',' | '，' | '、' | ';' | '；' | '/' | '|'))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// 白名单排序后的规范文风
pub fn parse_style_list(raw: &str) -> Vec<String> {
    let mut found = Vec::new();
    for p in split_raw(raw) {
        let n = normalize_style(&p);
        if !n.is_empty() && !found.iter().any(|x| x == &n) {
            found.push(n);
        }
    }
    CANON_STYLES
        .iter()
        .filter(|c| found.iter().any(|x| x == *c))
        .map(|s| (*s).to_string())
        .collect()
}

pub fn parse_style_values(values: &[String]) -> Vec<String> {
    parse_style_list(&values.join(","))
}

pub fn format_style_list(styles: &[String]) -> String {
    parse_style_list(&styles.join(",")).join(", ")
}

pub fn merge_style_strings(a: &str, b: &str) -> String {
    format_style_list(
        &split_raw(a)
            .into_iter()
            .chain(split_raw(b))
            .collect::<Vec<_>>(),
    )
}

/// 规范文风种子起步写法（缺条时写入；已有长文不覆盖）
pub fn seed_starter_content(title: &str) -> &'static str {
    match title {
        "含蓄" => {
            "用暗示、停顿、省略号与身体反应写欲望；少直呼器官与动作名。对白拐弯，留白让读者补完。过程可写，措辞克制。"
        }
        "露骨" => {
            "器官、动作、位置写清楚，不绕弯。感官与交合过程可正面描写，但仍避免无信息量堆砌脏话。"
        }
        "直言不讳" => {
            "叙述与对白不掩饰意图：想要就写想要，拒绝就写拒绝。少用比喻遮羞，句子短而准。"
        }
        "肮脏" => {
            "强调污渍、气味、体液、下贱感与羞耻对比；可用贬义自称与羞辱对白，但过程仍要可跟。"
        }
        "细腻" => {
            "拉长时间轴：触感、温度、呼吸、肌肉反应分层写。同一动作多感官回环，忌一笔带过。"
        }
        "粗口" => {
            "对白可带脏话、口头羞辱与粗俗称呼；叙述可夹少量脏词，但不要句句脏词淹没信息。"
        }
        "克制" => {
            "篇幅与尺度都收：少写、写准、点到即止。高潮与暴露可发生，但句子短、余韵留给下一拍。"
        }
        "淫秽" => {
            "以情色快感与亵渎感为主调；描写可浓、可脏、可反复强调敏感部位与反应，忌空喊玩法名。"
        }
        _ => "按本卡标题约束叙述与对白的表述方式；过程须可跟，禁止只点名。",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alias_blunt_and_dirty() {
        assert_eq!(normalize_style("直白"), "直言不讳");
        assert_eq!(normalize_style("脏话"), "粗口");
        assert_eq!(normalize_style("dirty"), "肮脏");
    }

    #[test]
    fn drops_unknown() {
        assert!(normalize_style("玄幻").is_empty());
        assert!(parse_style_list("foo, bar").is_empty());
    }

    #[test]
    fn merge_unions_and_dedupes() {
        let s = merge_style_strings("含蓄, 肮脏", "肮脏；露骨");
        assert_eq!(s, "含蓄, 露骨, 肮脏");
    }
}
