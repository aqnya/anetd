use adblock::engine::Engine;
use adblock::request::Request;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub enum FilterAction {
    Allow,
    Block,
    Redirect(String),
}

#[derive(Clone)]
pub struct RuleSet {
    pub engine: Arc<Engine>,
    pub watched_files: Vec<(String, String)>, // (file_path, sha256_hash)
}

impl std::fmt::Debug for RuleSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuleSet")
            .field("watched_files_count", &self.watched_files.len())
            .finish()
    }
}

impl RuleSet {
    pub fn new() -> Self {
        use adblock::lists::ParseOptions;
        let engine = Engine::from_rules(std::iter::empty::<String>(), ParseOptions::default());
        Self {
            engine: Arc::new(engine),
            watched_files: Vec::new(),
        }
    }

    pub fn matches(&self, url: &str, source_domain: &str, resource_type: &str) -> FilterAction {
        let request = match Request::new(url, source_domain, resource_type) {
            Ok(req) => req,
            Err(_) => return FilterAction::Allow,
        };

        let result = self.engine.check_network_request(&request);

        if result.matched {
            if result.exception.is_some() {
                return FilterAction::Allow;
            }
            if let Some(redirect_url) = result.redirect {
                return FilterAction::Redirect(redirect_url);
            }
            FilterAction::Block
        } else {
            FilterAction::Allow
        }
    }
}
