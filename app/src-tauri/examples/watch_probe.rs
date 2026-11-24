//! Times the file watcher: watches a folder recursively and prints `event <epoch-ms>`
//! for the first event after at least 500 ms of quiet. `scripts/measure.sh` pairs
//! each line with the time a `daiku gate` finished. Exits when stdin closes.
//!
//! Run: cargo run --release --example watch_probe -- <folder>

use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use notify::{RecommendedWatcher, RecursiveMode, Watcher};

const QUIET: Duration = Duration::from_millis(500);

fn epoch_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn main() {
    let Some(root) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("usage: watch_probe <folder>");
        std::process::exit(2);
    };

    let (tx, rx) = mpsc::channel();
    let events = tx.clone();
    let mut watcher = RecommendedWatcher::new(
        move |res: notify::Result<notify::Event>| {
            if res.is_ok() {
                let _ = events.send(Some(epoch_ms()));
            }
        },
        notify::Config::default(),
    )
    .expect("create watcher");
    watcher
        .watch(&root, RecursiveMode::Recursive)
        .expect("watch folder");

    // The end of stdin ends the probe.
    std::thread::spawn(move || {
        let mut sink = [0u8; 64];
        while matches!(std::io::stdin().read(&mut sink), Ok(n) if n > 0) {}
        let _ = tx.send(None);
    });

    println!("ready {}", epoch_ms());
    let _ = std::io::stdout().flush();

    let mut last: Option<Instant> = None;
    while let Ok(Some(at)) = rx.recv() {
        let now = Instant::now();
        if last.is_none_or(|l| now.duration_since(l) >= QUIET) {
            println!("event {at}");
            let _ = std::io::stdout().flush();
        }
        last = Some(now);
    }
}
