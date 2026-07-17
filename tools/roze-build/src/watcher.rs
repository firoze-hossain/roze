// src/watcher.rs
use crate::build::Builder;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher as NotifyWatcher};
use std::path::Path;
use std::sync::mpsc::channel;
use anyhow::Result;

pub struct Watcher {
    builder: Builder,
}

impl Watcher {
    pub fn new(builder: Builder) -> Self {
        Self { builder }
    }

    pub fn watch(&self) -> Result<()> {
        let (tx, rx) = channel();

        let mut watcher = RecommendedWatcher::new(
            move |res: Result<notify::Event, notify::Error>| {
                if let Ok(event) = res {
                    if let notify::EventKind::Modify(_) = event.kind {
                        let _ = tx.send(());
                    }
                }
            },
            Config::default(),
        )?;

        // Watch source directory
        let src_path = Path::new("src");
        if src_path.exists() {
            watcher.watch(src_path, RecursiveMode::Recursive)?;
        }

        println!("👁️ Watching for changes... (Press Ctrl+C to stop)");

        // Clone builder for reuse
        let mut builder = self.builder.clone();

        loop {
            match rx.recv() {
                Ok(_) => {
                    println!("\n🔄 Changes detected, rebuilding...");
                    if let Err(e) = builder.build() {
                        println!("❌ Build failed: {}", e);
                    } else {
                        println!("✅ Rebuild successful!");
                    }
                    println!("👁️ Watching for changes...");
                }
                Err(_) => break,
            }
        }

        Ok(())
    }
}