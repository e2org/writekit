extern crate clap;
extern crate notify;

use std::env;
use std::sync::mpsc::channel;
use std::thread::sleep;
use std::time::Duration;

use clap::clap_app;
use notify::DebouncedEvent::{Create, Write};
use notify::{watcher, RecursiveMode, Watcher};

use writekit::{handle_write, Options};

const VERSION: &'static str = env!("CARGO_PKG_VERSION");
const DESCRIPTION: &'static str = env!("CARGO_PKG_DESCRIPTION");
const AUTHOR: &'static str = env!("CARGO_PKG_AUTHORS");

fn main() {
    let opt = Options::new(
        clap_app!(writekit =>
            (version: VERSION)
            (author: AUTHOR)
            (about: DESCRIPTION)
            (@arg TARGET: "directory or file to watch for changes")
            (@arg verbose: --verbose -v)
            (@arg quiet: --quiet -q)
        )
        .get_matches(),
    );

    if opt.verbose {
        println!("{:#?}", opt);
    }

    // https://docs.rs/notify/4.0.10/notify/#default-debounced-api
    // Create a channel to receive the events.
    let (transmitter, receiver) = channel();
    // Create a watcher object, delivering debounced events.
    // The notification back-end is selected based on the platform.
    let mut watcher = watcher(transmitter, Duration::from_secs(1)).unwrap();
    // Add a path to be watched. All files and directories at that path and
    // below will be monitored for changes.
    watcher
        .watch(opt.target.clone(), RecursiveMode::Recursive)
        .unwrap();

    loop {
        match receiver.recv() {
            Ok(event) => {
                match event {
                    Create(path) | Write(path) => {
                        // New file may have been created -- ensure it's watched:
                        watcher
                            .watch(path.clone(), RecursiveMode::Recursive)
                            .unwrap();

                        // Generate all downstream files from changed file:
                        match handle_write(path, opt.verbose, opt.quiet) {
                            Ok(()) => (),
                            Err(error) => eprintln!("error: {:?}", error),
                        };
                    }
                    _ => (),
                }
            }
            Err(error) => eprintln!("error: {:?}", error),
        }
    }
}
