extern crate clap;
extern crate notify;

use std::env;
use std::sync::{Arc, Mutex};

use clap::clap_app;
use notify::DebouncedEvent::{Create, Write};

use writekit::{handle_write, Args, Loading, Monitor};

// Get config values directly from Cargo.toml so they _never_ get out of sync:
const VERSION: &str = env!("CARGO_PKG_VERSION");
const DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");
const AUTHOR: &str = env!("CARGO_PKG_AUTHORS");

fn main() {
    // Parse arguments from command line via https://github.com/clap-rs/clap
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

    // Mutable Loading instance must be shared between multiple handle_write
    // calls in closure below, so use mutex to safely manage mutable sharing.
    let loading_arc = Arc::new(Mutex::new(Loading::new().clear()));

    Monitor::new(1_000) // debounce milliseconds
        // On Monitor initialization error -- panic to exit script:
        .unwrap_or_else(|error| panic!("error: {:?}", error))
        .path(&args.target)
        .watch(|event_result| match event_result {
            Ok(event) => {
                if args.verbose {
                    println!("[file monitor event] {:?}", event); // diagnostic
                }

                match event {
                    Create(path) | Write(path) => {
                        // Get & lock Loading instance:
                        match loading_arc.lock() {
                            Ok(mut loading) => {
                                // Generate all downstream files from changed file:
                                handle_write(
                                    &path,
                                    &mut loading,
                                    args.display,
                                    args.verbose,
                                    args.quiet,
                                )
                                .unwrap_or_else(|error| eprintln!("error: {:?}", error));
                            }
                            Err(error) => eprintln!("error: {:?}", error),
                        }
                    }
                    _ => (),
                }
            }
            // On error during Monitor watch loop -- emit error message but continue watching:
            Err(error) => eprintln!("error: {:?}", error),
        })
        // On Monitor watch start error -- panic to exit script:
        .unwrap_or_else(|error| panic!("error: {:?}", error));
}
