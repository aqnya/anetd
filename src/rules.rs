use arc_swap::ArcSwap;
use log::{error, info, warn};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use adblock::engine::Engine;
use adblock::lists::ParseOptions;
use adblock::request::Request;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq)]
pub enum FilterAction {
    Allow,
    Block,
    Redirect(String),
}

#[derive(Clone)]
pub struct RuleSet {
    pub engine: Arc<Engine>,
    pub watched_files: Vec<(String, String)>,
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

fn calculate_file_hash(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Some(hex::encode(hasher.finalize()))
}

fn collect_files_and_hashes(path_str: &str, files: &mut Vec<(String, String)>) {
    if path_str.contains(',') {
        for sub_path in path_str.split(',') {
            collect_files_and_hashes(sub_path.trim(), files);
        }
        return;
    }

    let path = Path::new(path_str);
    if !path.exists() {
        warn!("[rules] path does not exist: {}", path_str);
        return;
    }

    if path.is_file() {
        if let Some(hash_str) = calculate_file_hash(path) {
            files.push((path_str.to_string(), hash_str));
        }
    } else if path.is_dir() {
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                if let Some(s) = entry.path().to_str() {
                    collect_files_and_hashes(s, files);
                }
            }
        }
    }
}

pub fn load_rules(path_str: &str) -> RuleSet {
    let mut watched_files = Vec::new();
    collect_files_and_hashes(path_str, &mut watched_files);

    if watched_files.is_empty() {
        warn!(
            "[rules] no valid rule files found for path: {}, using empty engine",
            path_str
        );
        return RuleSet::new();
    }

    let mut all_lines = Vec::new();

    for (file_path, _) in &watched_files {
        match std::fs::read_to_string(file_path) {
            Ok(text) => {
                let lines_count_before = all_lines.len();
                for line in text.lines() {
                    let s = line.trim();
                    if !s.is_empty() && !s.starts_with('!') && !s.starts_with('[') {
                        all_lines.push(s.to_string());
                    }
                }
                info!(
                    "[rules] loaded {} rules from {}",
                    all_lines.len() - lines_count_before,
                    file_path
                );
            }
            Err(e) => {
                error!("[rules] failed to read file {file_path}: {e}");
            }
        }
    }

    info!(
        "[rules] compiling total {} rules into Adblock engine...",
        all_lines.len()
    );
    let engine = Engine::from_rules(all_lines, ParseOptions::default());

    RuleSet {
        engine: Arc::new(engine),
        watched_files,
    }
}

pub fn spawn_reload_watcher(path: String, store: &'static ArcSwap<RuleSet>) {
    tokio::spawn(async move {
        match try_inotify_watcher(path.clone(), store).await {
            Ok(()) => {}
            Err(e) => {
                warn!("[rules] inotify unavailable ({e}), file change detection disabled");
            }
        }
    });
}

async fn try_inotify_watcher(
    path: String,
    store: &'static ArcSwap<RuleSet>,
) -> Result<(), Box<dyn std::error::Error>> {
    use inotify::{EventMask, Inotify, WatchMask};

    let inotify = Inotify::init()?;

    for (file_path, _) in &store.load().watched_files {
        let p = Path::new(file_path);
        let watch_target = if p.is_dir() {
            p
        } else {
            p.parent().unwrap_or(p)
        };
        inotify
            .watches()
            .add(
                watch_target,
                WatchMask::CLOSE_WRITE
                    | WatchMask::CREATE
                    | WatchMask::DELETE
                    | WatchMask::MOVED_TO,
            )
            .map_err(|e| {
                warn!("[rules] failed to watch {}: {e}", watch_target.display());
                e
            })?;
    }

    let mut buffer = vec![0u8; 4096];
    let mut event_stream = inotify.into_event_stream(&mut buffer)?;

    info!("[rules] inotify watcher active");

    while let Some(event_res) = tokio_stream::StreamExt::next(&mut event_stream).await {
        let event = event_res?;

        if event.mask.contains(EventMask::ISDIR) {
            continue;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;

        let path_clone = path.clone();
        let mut new_rules = tokio::task::spawn_blocking(move || load_rules(&path_clone)).await?;
        let mut current_files = store.load().watched_files.clone();

        new_rules.watched_files.sort_unstable();
        current_files.sort_unstable();

        if new_rules.watched_files == current_files {
            info!("[rules] inotify: no content hash change, skipping reload");
            continue;
        }

        info!("[rules] inotify: file content change detected, reloading...");
        store.store(Arc::new(new_rules));
        info!("[rules] reloaded rules successfully");
    }

    Ok(())
}
