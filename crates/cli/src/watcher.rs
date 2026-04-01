use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use notify::{EventKind, RecursiveMode, Watcher as _};
use yansi::Paint;

const DEBOUNCE_TIMEOUT: Duration = Duration::from_millis(10);
const MAX_HOLD: Duration = Duration::from_secs(3);
const RESCAN_THRESHOLD: usize = 10_000;

// Change events

/// A normalized file system change event.
#[derive(Debug)]
pub enum ChangeEvent {
    Created {
        path: PathBuf,
        kind: CreatedKind,
    },
    Changed {
        path: PathBuf,
        kind: ChangedKind,
    },
    Deleted {
        path: PathBuf,
        kind: DeletedKind,
    },
    /// The watcher lost sync with the filesystem - a full re-scan is needed.
    Rescan,
}

#[derive(Debug)]
pub enum CreatedKind {
    File,
    Directory,
    Any,
}

#[derive(Debug)]
pub enum ChangedKind {
    FileContent,
    FileMetadata,
    Any,
}

#[derive(Debug)]
pub enum DeletedKind {
    File,
    Directory,
    Any,
}

impl ChangeEvent {
    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::Created { path, .. }
            | Self::Changed { path, .. }
            | Self::Deleted { path, .. } => Some(path),
            Self::Rescan => None,
        }
    }
}

// Debouncer

/// Accumulates raw notify events into a batch of `ChangeEvent`s.
struct Debouncer {
    events: Vec<ChangeEvent>,
    rescan: bool,
}

impl Debouncer {
    fn new() -> Self {
        Self {
            events: Vec::new(),
            rescan: false,
        }
    }

    fn add_notify_event(&mut self, event: notify::Event) {
        if self.rescan {
            return;
        }

        if event.need_rescan() || self.events.len() > RESCAN_THRESHOLD {
            self.events = Vec::new();
            self.rescan = true;
            return;
        }

        for path in event.paths {
            if !is_relevant_path(&path) {
                continue;
            }

            match event.kind {
                EventKind::Create(kind) => {
                    let created_kind = match kind {
                        notify::event::CreateKind::File => CreatedKind::File,
                        notify::event::CreateKind::Folder => CreatedKind::Directory,
                        _ => file_type_from_path(&path),
                    };
                    self.events.push(ChangeEvent::Created {
                        path,
                        kind: created_kind,
                    });
                }

                EventKind::Modify(kind) => match kind {
                    notify::event::ModifyKind::Data(_) => {
                        self.events.push(ChangeEvent::Changed {
                            path,
                            kind: ChangedKind::FileContent,
                        });
                    }
                    notify::event::ModifyKind::Metadata(_) => {
                        if path.is_file() {
                            self.events.push(ChangeEvent::Changed {
                                path,
                                kind: ChangedKind::FileMetadata,
                            });
                        }
                    }
                    notify::event::ModifyKind::Name(rename_mode) => match rename_mode {
                        notify::event::RenameMode::From => {
                            self.events.push(ChangeEvent::Deleted {
                                path,
                                kind: DeletedKind::Any,
                            });
                        }
                        notify::event::RenameMode::To => {
                            let created_kind = file_type_from_path(&path);
                            self.events.push(ChangeEvent::Created {
                                path,
                                kind: created_kind,
                            });
                        }
                        notify::event::RenameMode::Both => {
                            // Rely on separate From/To events
                        }
                        notify::event::RenameMode::Any => {
                            // Can't tell direction - check filesystem state
                            if path.exists() {
                                let created_kind = file_type_from_path(&path);
                                self.events.push(ChangeEvent::Created {
                                    path,
                                    kind: created_kind,
                                });
                            } else {
                                self.events.push(ChangeEvent::Deleted {
                                    path,
                                    kind: DeletedKind::Any,
                                });
                            }
                        }
                        _ => {}
                    },
                    notify::event::ModifyKind::Any => {
                        if path.is_file() {
                            self.events.push(ChangeEvent::Changed {
                                path,
                                kind: ChangedKind::Any,
                            });
                        }
                    }
                    _ => {}
                },

                EventKind::Remove(kind) => {
                    let deleted_kind = match kind {
                        notify::event::RemoveKind::File => DeletedKind::File,
                        notify::event::RemoveKind::Folder => DeletedKind::Directory,
                        _ => DeletedKind::Any,
                    };
                    self.events.push(ChangeEvent::Deleted {
                        path,
                        kind: deleted_kind,
                    });
                }

                // Access events are not relevant
                EventKind::Access(_) | EventKind::Other => {}
                EventKind::Any => {}
            }
        }
    }

    fn into_events(self) -> Vec<ChangeEvent> {
        if self.rescan {
            vec![ChangeEvent::Rescan]
        } else {
            self.events
        }
    }
}

/// Determine the created kind by checking the filesystem.
fn file_type_from_path(path: &Path) -> CreatedKind {
    if path.is_file() {
        CreatedKind::File
    } else if path.is_dir() {
        CreatedKind::Directory
    } else {
        CreatedKind::Any
    }
}

/// Only care about `.st` files and `config.toml`.
fn is_relevant_path(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "st")
        || path.file_name().is_some_and(|name| name == "config.toml")
}

// Messages 

enum DebouncerMessage {
    Event(notify::Event),
    Flush,
}

// Watcher 

struct WatcherInner {
    _watcher: notify::RecommendedWatcher,
    sender: mpsc::Sender<DebouncerMessage>,
    thread: Option<std::thread::JoinHandle<()>>,
}

/// File system watcher with debouncing: `.st` and `config.toml` changes,
/// batched with a 10ms inactivity timeout and 3s max hold.
pub struct Watcher {
    inner: Option<WatcherInner>,
}

impl Watcher {
    /// Create a new watcher for `workspace`, calling `handler` with batched events.
    pub fn new(
        workspace: &Path,
        handler: impl Fn(Vec<ChangeEvent>) + Send + 'static,
    ) -> Self {
        let (tx, rx) = mpsc::channel::<DebouncerMessage>();

        let tx_notify = tx.clone();
        let watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            if let Ok(event) = res {
                let _ = tx_notify.send(DebouncerMessage::Event(event));
            }
        })
        .unwrap_or_else(|e| {
            eprintln!("{}{}", "watcher error: ".bold().red(), e);
            std::process::exit(1);
        });

        let thread = std::thread::spawn(move || {
            debouncer_loop(rx, handler);
        });

        let mut w = Watcher {
            inner: Some(WatcherInner {
                _watcher: watcher,
                sender: tx,
                thread: Some(thread),
            }),
        };

        // Start watching
        w.inner
            .as_mut()
            .unwrap()
            ._watcher
            .watch(workspace, RecursiveMode::Recursive)
            .unwrap_or_else(|e| {
                eprintln!("{}{}", "watcher error: ".bold().red(), e);
                std::process::exit(1);
            });

        w
    }

    /// Force flush any pending events.
    pub fn flush(&self) {
        if let Some(inner) = &self.inner {
            let _ = inner.sender.send(DebouncerMessage::Flush);
        }
    }

    /// Stop the watcher and wait for the debouncer thread to exit.
    pub fn stop(&mut self) {
        if let Some(inner) = self.inner.take() {
            // Drop watcher first to stop new events
            drop(inner._watcher);
            // Drop sender to close the channel
            drop(inner.sender);
            // Wait for debouncer thread
            if let Some(thread) = inner.thread {
                let _ = thread.join();
            }
        }
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        self.stop();
    }
}

/// The debouncer thread main loop.
fn debouncer_loop(
    rx: mpsc::Receiver<DebouncerMessage>,
    handler: impl Fn(Vec<ChangeEvent>),
) {
    loop {
        // Block until first event
        let msg = match rx.recv() {
            Ok(msg) => msg,
            Err(_) => return, // channel closed - shutdown
        };

        let mut debouncer = Debouncer::new();

        match msg {
            DebouncerMessage::Event(event) => debouncer.add_notify_event(event),
            DebouncerMessage::Flush => continue, // nothing to flush
        }

        // Accumulate: 10ms inactivity or 3s max hold
        let batch_start = Instant::now();
        loop {
            let remaining = MAX_HOLD.saturating_sub(batch_start.elapsed());
            if remaining.is_zero() {
                break;
            }

            let timeout = remaining.min(DEBOUNCE_TIMEOUT);
            match rx.recv_timeout(timeout) {
                Ok(DebouncerMessage::Event(event)) => {
                    debouncer.add_notify_event(event);
                }
                Ok(DebouncerMessage::Flush) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => break,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    // Shutdown - process remaining events and exit
                    let events = debouncer.into_events();
                    if !events.is_empty() {
                        handler(events);
                    }
                    return;
                }
            }
        }

        let events = debouncer.into_events();
        if !events.is_empty() {
            handler(events);
        }
    }
}

// Convenience

/// Watch a workspace and call `on_change` after each batch, after an
/// initial run; blocks forever.
pub fn watch_and_run(workspace: &Path, mut on_change: impl FnMut()) {
    // Initial run
    on_change();

    let (tx, rx) = mpsc::sync_channel::<()>(0);

    let _watcher = Watcher::new(workspace, move |_events| {
        let _ = tx.send(());
    });

    println!("{}", "watching for changes...".dim());

    while rx.recv().is_ok() {
        println!("\n{}", "file change detected, re-running...".dim());
        on_change();
        println!("{}", "watching for changes...".dim());
    }
}
