// src/rules.rs
use arc_swap::ArcSwap;
use log::{error, info};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use std::time::SystemTime;

#[derive(Debug, Clone, PartialEq)]
pub enum FilterAction {
    Allow,
    Block,
    Redirect(String),
    Fake(String),
}

#[derive(Debug, Clone)]
pub enum PatternType {
    /// 全匹配或传统模式（由原本的 pattern 函数处理）
    Classic(Option<String>),
    /// ABP 域规则：||example.com^ 匹配 example.com 及其所有子域名
    AdblockDomain(String),
    /// 通配符模式：如 *-ad.sm.cn*，转换为简单的前后缀或包含匹配
    Wildcard {
        prefix_star: bool,
        content: String,
        suffix_star: bool,
    },
}

#[derive(Debug, Clone)]
pub struct FilterRule {
    pub pattern_type: PatternType,
    pub action: FilterAction,
}

impl FilterRule {
    pub fn matches(&self, hostname: &str) -> bool {
        let h = hostname.to_lowercase();

        match &self.pattern_type {
            PatternType::Classic(None) => true,

            PatternType::Classic(Some(p)) => {
                let p = p.to_lowercase();
                if h == p {
                    return true;
                }
                if h.len() > p.len() && h.ends_with(&p) && h[..h.len() - p.len()].ends_with('.') {
                    return true;
                }
                false
            }

            PatternType::AdblockDomain(domain) => {
                let p = domain.to_lowercase();
                if h == p {
                    return true;
                }
                // 匹配子域名，例如 h="test.8le8le.com", p="8le8le.com"
                if h.len() > p.len() && h.ends_with(&p) && h[..h.len() - p.len()].ends_with('.') {
                    return true;
                }
                false
            }

            PatternType::Wildcard {
                prefix_star,
                content,
                suffix_star,
            } => {
                let p = content.to_lowercase();
                match (prefix_star, suffix_star) {
                    (true, true) => h.contains(&p),
                    (true, false) => h.ends_with(&p),
                    (false, true) => h.starts_with(&p),
                    (false, false) => h == p,
                }
            }
        }
    }
}

/// 传统规则的 pattern 解析
fn parse_classic_pattern(s: &str) -> Option<String> {
    if s == "*" || s == ".*" {
        None
    } else if s.starts_with("*.") {
        Some(s[2..].into())
    } else if s.starts_with('.') {
        Some(s[1..].into())
    } else {
        Some(s.into())
    }
}

/// 解析单条规则文本
fn parse_single_pattern(s: &str) -> PatternType {
    // 1. 处理 ||domain^ 格式
    if s.starts_with("||") && s.ends_with('^') {
        let domain = &s[2..s.len() - 1];
        return PatternType::AdblockDomain(domain.to_string());
    }

    // 2. 处理带 * 的通配符格式 (如 *-ad.sm.cn*)
    if s.contains('*') {
        let prefix_star = s.starts_with('*');
        let suffix_star = s.ends_with('*');

        // 移除首尾的 *
        let start_idx = if prefix_star { 1 } else { 0 };
        let end_idx = if suffix_star { s.len() - 1 } else { s.len() };

        let content = if start_idx < end_idx {
            s[start_idx..end_idx].to_string()
        } else {
            String::new()
        };

        return PatternType::Wildcard {
            prefix_star,
            content,
            suffix_star,
        };
    }

    // 3. 回退到原有经典模式
    PatternType::Classic(parse_classic_pattern(s))
}

pub fn parse_rules(text: &str) -> Vec<FilterRule> {
    let mut rules = Vec::new();
    for (lineno, raw) in text.lines().enumerate() {
        let line = match raw.find('#') {
            Some(i) => &raw[..i],
            None => raw,
        }
        .trim();
        if line.is_empty() {
            continue;
        }

        let cols: Vec<&str> = line.split_whitespace().collect();

        // 如果包含传统动作前缀 (allow/block/fake/redirect)
        let rule = match cols.as_slice() {
            ["allow", pat] => FilterRule {
                pattern_type: PatternType::Classic(parse_classic_pattern(pat)),
                action: FilterAction::Allow,
            },
            ["block", pat] => FilterRule {
                pattern_type: PatternType::Classic(parse_classic_pattern(pat)),
                action: FilterAction::Block,
            },
            ["fake", pat, ip] => FilterRule {
                pattern_type: PatternType::Classic(parse_classic_pattern(pat)),
                action: FilterAction::Fake((*ip).into()),
            },
            ["redirect", pat, dest] => FilterRule {
                pattern_type: PatternType::Classic(parse_classic_pattern(pat)),
                action: FilterAction::Redirect((*dest).into()),
            },
            // 如果没有匹配到传统前缀，说明是整行纯规则（如你提供的新列表），默认作 Block 处理
            _ => {
                // 再次确保不是乱填的多列数据
                if cols.len() == 1 {
                    FilterRule {
                        pattern_type: parse_single_pattern(cols[0]),
                        action: FilterAction::Block,
                    }
                } else {
                    eprintln!("[rules] line {}: unrecognized: {:?}", lineno + 1, line);
                    continue;
                }
            }
        };
        rules.push(rule);
    }

    // 如果没有任何全放行规则，末尾补一个默认放行
    if !rules
        .iter()
        .any(|r| matches!(r.pattern_type, PatternType::Classic(None)))
    {
        rules.push(FilterRule {
            pattern_type: PatternType::Classic(None),
            action: FilterAction::Allow,
        });
    }
    rules
}

pub fn load_rules(path: &str) -> Vec<FilterRule> {
    match std::fs::read_to_string(path) {
        Ok(text) => {
            let rules = parse_rules(&text);
            info!("[rules] loaded {} rules from {}", rules.len(), path);
            rules
        }
        Err(e) => {
            error!("[rules] failed to load {path}: {e}, using default allow-all");
            vec![FilterRule {
                pattern_type: PatternType::Classic(None),
                action: FilterAction::Allow,
            }]
        }
    }
}

pub fn mtime(path: &str) -> Option<SystemTime> {
    std::fs::metadata(path).ok()?.modified().ok()
}

pub fn spawn_reload_watcher(path: String, store: &'static ArcSwap<Vec<FilterRule>>) {
    thread::spawn(move || {
        let mut last = mtime(&path);
        loop {
            thread::sleep(Duration::from_secs(3));
            if let Some(cur) = mtime(&path) {
                if Some(cur) != last {
                    last = Some(cur);
                    store.store(Arc::new(load_rules(&path)));
                    info!("[rules] reloaded from {}", path);
                }
            }
        }
    });
}
