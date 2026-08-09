
use crate::ui::app::events::AppEvent;
use notify::{
    Config, Event as NEvent, EventKind, RecommendedWatcher, RecursiveMode, Watcher,
};
use notify::event::{CreateKind, RemoveKind};
use std::{path::PathBuf, sync::mpsc, time::Duration, thread};

/// Starts a debounced FS watcher on `root`.
/// Returns the watcher handle — drop it to stop watching.
pub fn start_watcher(
    root: PathBuf,
    tx:   mpsc::Sender<AppEvent>,
) -> RecommendedWatcher {
    let (ntx, nrx) = mpsc::channel::<Result<NEvent, notify::Error>>();

    let mut watcher = RecommendedWatcher::new(ntx, Config::default()
        .with_poll_interval(Duration::from_millis(200)))
        .expect("Failed to create FS watcher");

    watcher.watch(&root, RecursiveMode::Recursive)
        .expect("Failed to watch root path");

    // Watcher dispatch thread
    thread::spawn(move || {
        for res in nrx {
            let Ok(ev) = res else { continue };

            let app_event = match ev.kind {
                EventKind::Create(CreateKind::File) => {
                    ev.paths.into_iter().map(AppEvent::FsCreate).next()
                }
                EventKind::Remove(RemoveKind::File) => {
                    ev.paths.into_iter().map(AppEvent::FsRemove).next()
                }
                EventKind::Modify(_) => {
                    ev.paths.into_iter().map(AppEvent::FsModify).next()
                }
                _ => None,
            };

            if let Some(e) = app_event {
                if tx.send(e).is_err() { break; }
            }
        }
    });

    watcher
}
