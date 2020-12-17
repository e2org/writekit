extern crate clap;
extern crate notify;

use std::env;
use std::sync::mpsc::channel;
use std::time::Duration;

use clap::clap_app;
use notify::DebouncedEvent::{Create, Write};
use notify::{watcher, RecursiveMode, Watcher};

use writekit::{handle_write, Args, Loading};

// Get config values directly from Cargo.toml so they _never_ get out of sync:
const VERSION: &'static str = env!("CARGO_PKG_VERSION");
const DESCRIPTION: &'static str = env!("CARGO_PKG_DESCRIPTION");
const AUTHOR: &'static str = env!("CARGO_PKG_AUTHORS");

fn main() {
    // Parse arguments from command line via Clap:
    // https://github.com/clap-rs/clap
    let args = Args::new(
        clap_app!(writekit =>
            // Use config values from Cargo.toml:
            (version: VERSION)
            (author: AUTHOR)
            (about: DESCRIPTION)
            // Positional argument:
            (@arg TARGET: "directory or file to watch for changes")
            // Boolean arguments (flags):
            (@arg display: --display -d)
            (@arg verbose: --verbose -v)
            (@arg quiet: --quiet -q)
        )
        // Args constructor accepts a clap::ArgMatches object:
        .get_matches(),
    )
    // Args constructor will error if no target directory was provided and
    // it is unable to determine current working directory of script.
    // In this case, print the error and exit immediately (!panic):
    .unwrap_or_else(|error| panic!("error: {:?}", error));

    // If verbose mode requested, print info line with argument values.
    // Formatting handled via Args::fmt -- implementation of Display trait.
    if args.verbose {
        println!("{}", args);
    }

    // Watch for file changes in target directory via Notify:
    // https://docs.rs/notify/4.0.10/notify/#default-debounced-api

    // Create a channel to receive the events:
    let (transmitter, receiver) = channel();

    // Create a watcher object, delivering debounced events:
    // (the notification back-end is selected based on the platform)
    let mut watcher = watcher(transmitter, Duration::from_secs(1)).unwrap();

    // Add a path to be watched:
    // (all files and directories at that path and below will be monitored)
    watcher
        .watch(args.target, RecursiveMode::Recursive)
        .unwrap_or_else(|error| panic!("error: {:?}", error));

    let mut loading = Loading::new();
    loop {
        match receiver.recv() {
            Ok(event) => {
                if args.verbose {
                    println!("[notify] {:?}", event);
                }

                match event {
                    Create(path) | Write(path) => {
                        // Generate all downstream files from changed file:
                        handle_write(&path, &mut loading, args.display, args.verbose, args.quiet)
                            .unwrap_or_else(|error| eprintln!("error: {:?}", error));

                        // New file may have been created -- ensure it's watched:
                        watcher
                            .watch(path, RecursiveMode::Recursive)
                            .unwrap_or_else(|error| eprintln!("error: {:?}", error));
                    }
                    _ => (),
                }
            }
            Err(error) => eprintln!("error: {:?}", error),
        }
    }
}
