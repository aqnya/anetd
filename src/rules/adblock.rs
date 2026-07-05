use std::collections::HashSet;

/// Outcome of a rule-matching operation.
#[derive(Debug, Clone, PartialEq)]
pub enum FilterAction {
    Allow,
    Block,
}

/// Lightweight domain-based rule set.
///
/// Replaces the `adblock` crate with a minimal engine that only handles
/// network-level (DNS) filtering.  It understands the common Adblock filter
/// syntaxes that are relevant for domain blocking:
///
/// * `||example.com^`        – block the domain and all sub-domains
/// * `@@||example.com^`      – exception (allow) for the domain
///
/// Cosmetic / element-hiding rules (`$generichide`, `$csp`, `#@%`, etc.) are
/// silently ignored because they have no meaning at the DNS level.
#[derive(Clone)]
pub struct RuleSet {
    /// Suffix-based blocklist: "example.com" matches exactly "example.com"
    /// **and** any sub-domain like "ads.example.com".
    block_suffix: HashSet<String>,

    /// Suffix-based allowlist (exceptions).
    allow_suffix: HashSet<String>,

    /// Metadata about the watched rule files (path → sha256).
    pub watched_files: Vec<(String, String)>,
}

impl std::fmt::Debug for RuleSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuleSet")
            .field("block_rules", &self.block_suffix.len())
            .field("allow_rules", &self.allow_suffix.len())
            .field("watched_files_count", &self.watched_files.len())
            .finish()
    }
}

impl RuleSet {
    /// Create an empty rule set with no rules loaded.
    pub fn new() -> Self {
        Self {
            block_suffix: HashSet::new(),
            allow_suffix: HashSet::new(),
            watched_files: Vec::new(),
        }
    }

    /// Build a rule set from the raw filter lines (one rule per line).
    ///
    /// Lines starting with `!` (comments) or `[` (headers) are skipped.
    /// Only `||…^` and `@@||…^` patterns are extracted; everything else
    /// is ignored.
    pub fn from_rules(lines: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        let mut block_suffix: HashSet<String> = HashSet::new();
        let mut allow_suffix: HashSet<String> = HashSet::new();

        for line in lines {
            let line = line.as_ref().trim();
            if line.is_empty() || line.starts_with('!') || line.starts_with('[') {
                continue;
            }

            let (is_exception, rest) = if let Some(r) = line.strip_prefix("@@") {
                (true, r)
            } else {
                (false, line)
            };

            // Only handle "||…^" patterns; everything else is cosmetic /
            // element-hiding and irrelevant for DNS-level blocking.
            let domain = if let Some(d) = extract_domain_from_rule(rest) {
                d
            } else {
                continue;
            };

            if is_exception {
                allow_suffix.insert(domain);
            } else {
                block_suffix.insert(domain);
            }
        }

        // Remove any blocked domain that also appears in the allowlist.
        // (This handles `@@||example.com^` overriding `||example.com^`.)
        for d in &allow_suffix {
            block_suffix.remove(d);
        }

        Self {
            block_suffix,
            allow_suffix,
            watched_files: Vec::new(),
        }
    }

    /// Check whether a URL should be blocked.
    ///
    /// The `url` is expected to be a pseudo-URL like `"https://example.com/"`
    /// (as produced by `format_pseudo_url`).  `source_domain` and
    /// `resource_type` are accepted for API compatibility but are currently
    /// unused (DNS-level filtering does not depend on the source context).
    pub fn matches(&self, url: &str, _source_domain: &str, _resource_type: &str) -> FilterAction {
        let hostname = extract_hostname(url);
        if hostname.is_empty() {
            return FilterAction::Allow;
        }

        // 1) Check allowlist first (exceptions always win).
        if domain_in_suffix_set(&self.allow_suffix, hostname) {
            return FilterAction::Allow;
        }

        // 2) Check blocklist.
        if domain_in_suffix_set(&self.block_suffix, hostname) {
            return FilterAction::Block;
        }

        FilterAction::Allow
    }

    /// Number of blocking rules loaded.
    #[allow(dead_code)]
    pub fn block_count(&self) -> usize {
        self.block_suffix.len()
    }

    /// Number of exception rules loaded.
    #[allow(dead_code)]
    pub fn allow_count(&self) -> usize {
        self.allow_suffix.len()
    }
}

/// Extract a bare hostname from a (pseudo-)URL.
///
/// Handles:
/// * `"https://example.com/"`  → `"example.com"`
/// * `"example.com"`           → `"example.com"`
/// * `""`                      → `""`
fn extract_hostname(url: &str) -> &str {
    let s = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    let s = s.trim_end_matches('/');
    // Strip anything after the first '/' (path), '?' (query), or ':' (port).
    s.split(['/', '?', ':']).next().unwrap_or(s)
}

/// Try to extract a domain from a filter rule.
///
/// Returns `Some("example.com")` for the pattern `||example.com^`.
/// Returns `None` for patterns that are not domain-based (cosmetic rules,
/// element-hiding, options-only rules, etc.).
fn extract_domain_from_rule(rule: &str) -> Option<String> {
    let body = rule.strip_prefix("||")?;

    // Find the separator `^` (or end-of-string if no `^`).
    let domain_part = if let Some(pos) = body.find('^') {
        &body[..pos]
    } else {
        // Without `^`, the rule may still be a valid domain rule if it has
        // no path/options separators.
        if body.starts_with('$') || body.contains('#') {
            return None;
        }
        // Take the part before any option delimiter.
        body.split('$').next().unwrap_or(body)
    };

    if domain_part.is_empty() || domain_part.starts_with('$') {
        return None;
    }

    // Reject patterns that are clearly not domains (contain path chars).
    if domain_part.contains('/') || domain_part.contains('#') {
        return None;
    }

    // Normalise wildcard prefixes:
    //   ||*.example.com^   → example.com
    //   ||*foo.example.com^ → foo.example.com  (partial-label wildcard,
    //                            treated as if the leading * were absent)
    let domain = domain_part.trim_start_matches('*');
    // Also strip a leading dot that may follow the wildcard: "*.example.com"
    let domain = domain.strip_prefix('.').unwrap_or(domain);
    let domain = domain.trim_end_matches('.').to_lowercase();

    if domain.is_empty() {
        return None;
    }

    // Reject pure wildcard patterns like "*" that would match everything.
    if domain == "*" {
        return None;
    }

    Some(domain)
}

/// Check if `hostname` is covered by a suffix set entry.
///
/// `||example.com^` semantics:
/// * Exact match: `hostname == "example.com"`
/// * Sub-domain match: `hostname` ends with `".example.com"`
fn domain_in_suffix_set(set: &HashSet<String>, hostname: &str) -> bool {
    if set.is_empty() {
        return false;
    }

    let hostname = hostname.to_lowercase();

    // Walk parent domains from most-specific to least.
    let mut current = hostname.as_str();
    loop {
        if set.contains(current) {
            return true;
        }
        // Strip one label.
        match current.find('.') {
            Some(pos) => current = &current[pos + 1..],
            None => break,
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_hostname_basic() {
        assert_eq!(extract_hostname("https://example.com/"), "example.com");
        assert_eq!(extract_hostname("http://foo.bar/"), "foo.bar");
        assert_eq!(extract_hostname("bare.domain"), "bare.domain");
        assert_eq!(extract_hostname(""), "");
    }

    #[test]
    fn parse_domain_rule() {
        assert_eq!(
            extract_domain_from_rule("||example.com^"),
            Some("example.com".into())
        );
        assert_eq!(
            extract_domain_from_rule("||ads.example.com^"),
            Some("ads.example.com".into())
        );
        // trailing dot
        assert_eq!(
            extract_domain_from_rule("||example.com.^"),
            Some("example.com".into())
        );
        // with $domain= modifier – still extract domain
        assert_eq!(
            extract_domain_from_rule("||example.com^$domain=~foo.com"),
            Some("example.com".into())
        );
    }

    #[test]
    fn parse_non_domain_rules_ignored() {
        // cosmetic rules
        assert_eq!(extract_domain_from_rule("##.banner"), None);
        assert_eq!(extract_domain_from_rule("$generichide"), None);
        assert_eq!(extract_domain_from_rule(".ad-banner#$#"), None);
        // rule with path
        assert_eq!(extract_domain_from_rule("||example.com/path/to/ad"), None);
        // pure wildcard
        assert_eq!(extract_domain_from_rule("||*^"), None);
    }

    #[test]
    fn suffix_matching() {
        let mut set = HashSet::new();
        set.insert("example.com".to_string());
        set.insert("doubleclick.net".to_string());

        assert!(domain_in_suffix_set(&set, "example.com"));
        assert!(domain_in_suffix_set(&set, "ads.example.com"));
        assert!(domain_in_suffix_set(&set, "tracker.ads.example.com"));
        assert!(!domain_in_suffix_set(&set, "notexample.com"));
        assert!(!domain_in_suffix_set(&set, "com"));
        assert!(domain_in_suffix_set(&set, "doubleclick.net"));
        assert!(domain_in_suffix_set(&set, "stats.doubleclick.net"));
    }

    #[test]
    fn empty_ruleset_allows_all() {
        let rs = RuleSet::new();
        assert_eq!(rs.matches("https://evil.com/", "", ""), FilterAction::Allow);
    }

    #[test]
    fn block_and_allow() {
        let rs = RuleSet::from_rules(["||evil.com^", "||doubleclick.net^", "@@||good.evil.com^"]);

        // Blocked domains
        assert_eq!(rs.matches("https://evil.com/", "", ""), FilterAction::Block);
        assert_eq!(
            rs.matches("https://ads.evil.com/", "", ""),
            FilterAction::Block
        );
        assert_eq!(
            rs.matches("https://doubleclick.net/", "", ""),
            FilterAction::Block
        );

        // Exception overrides block
        assert_eq!(
            rs.matches("https://good.evil.com/", "", ""),
            FilterAction::Allow
        );

        // Unrelated domain passes
        assert_eq!(
            rs.matches("https://google.com/", "", ""),
            FilterAction::Allow
        );
    }

    #[test]
    fn allowlist_takes_priority() {
        let rs = RuleSet::from_rules(["||evil.com^", "@@||evil.com^"]);

        assert_eq!(rs.matches("https://evil.com/", "", ""), FilterAction::Allow);
        assert_eq!(
            rs.matches("https://ads.evil.com/", "", ""),
            FilterAction::Allow
        );
    }

    #[test]
    fn comments_and_headers_skipped() {
        let rs = RuleSet::from_rules([
            "[Adblock Plus 2.0]",
            "! This is a comment",
            "||evil.com^",
            "! Another comment",
        ]);

        assert_eq!(rs.block_count(), 1);
        assert_eq!(rs.matches("https://evil.com/", "", ""), FilterAction::Block);
    }

    #[test]
    fn cosmetic_rules_ignored() {
        let rs = RuleSet::from_rules([
            "||evil.com^",
            "##.banner-ad",
            "$generichide",
            "@@$generichide,domain=foo.com",
            "@@#.banner%20ad.$image",
        ]);

        // Only the ||evil.com^ rule should have been loaded.
        assert_eq!(rs.block_count(), 1);
        assert_eq!(rs.allow_count(), 0);
    }

    #[test]
    fn wildcard_domain_rules() {
        let rs = RuleSet::from_rules(["||*.diwodiwo.xyz^", "||*two-gun-volley.pages.dev^"]);

        assert_eq!(rs.block_count(), 2);

        // *.diwodiwo.xyz normalised to diwodiwo.xyz
        assert_eq!(
            rs.matches("https://diwodiwo.xyz/", "", ""),
            FilterAction::Block
        );
        assert_eq!(
            rs.matches("https://sub.diwodiwo.xyz/", "", ""),
            FilterAction::Block
        );

        // *two-gun-volley.pages.dev normalised to two-gun-volley.pages.dev
        assert_eq!(
            rs.matches("https://two-gun-volley.pages.dev/", "", ""),
            FilterAction::Block
        );
        assert_eq!(
            rs.matches("https://foo-two-gun-volley.pages.dev/", "", ""),
            FilterAction::Allow // partial-label wildcard not supported by suffix match
        );
    }
}
