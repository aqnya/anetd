use crate::rules::adblock::RuleSet;
use adblock::engine::Engine;
use adblock::lists::ParseOptions;
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::Arc;
use tracing::{error, info, warn};

fn calculate_file_hash(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Some(
        hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
    )
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
