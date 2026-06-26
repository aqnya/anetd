use arc_swap::ArcSwap;
use log::{info, warn};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use crate::rules::adblock::RuleSet;
use crate::rules::loader::load_rules;

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
