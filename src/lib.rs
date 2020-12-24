use std::env;
use std::error;
use std::fmt;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

use indicatif::{ProgressBar, ProgressStyle};
use notify::{self, DebouncedEvent as Event, FsEventWatcher, RecursiveMode, Watcher};

// Standard "error-boxing" Result type:
type Result<T> = ::std::result::Result<T, Box<dyn error::Error>>;

pub struct Args {
    pub target: PathBuf,
    pub display: bool,
    pub verbose: bool,
    pub quiet: bool,
}

impl fmt::Display for Args {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Arguments: target='{}' display={} verbose={} quiet={}",
            self.target.display(),
            self.display,
            self.verbose,
            self.quiet
        )
    }
}

impl Args {
    pub fn new(matches: clap::ArgMatches) -> Result<Args> {
        let display = matches.is_present("display");
        let verbose = matches.is_present("verbose");
        let quiet = matches.is_present("quiet");

        // Read "target" argument, a file or directory to be watched for changes.
        // If none provided, set to current directory (in which script was invoked).
        let target = match matches.value_of("TARGET") {
            Some(arg) => [arg].iter().collect(), // build PathBuf from arg string
            None => env::current_dir()?,         // use current directory if available
        };

        Ok(Args {
            target,
            display,
            verbose,
            quiet,
        })
    }
}

pub struct Loading<'a> {
    eta: Duration,
    delay: Duration,
    timer: Instant,
    thread: Option<Sender<()>>,
    clear: bool,
    chars: &'a str,
    template: &'a str,
}

impl<'a> Loading<'a> {
    pub fn new() -> Loading<'a> {
        Loading {
            eta: Duration::from_millis(10_000),
            delay: Duration::from_millis(100),
            timer: Instant::now(),
            thread: None,
            clear: false,
            chars: "``'",
            template: "{wide_bar:.cyan/blue}", // wide_bar : expand to pane width
        }
    }

    // Use Builder Pattern to configure Loading instance
    // e.g. Loading::new().clear().chars("++-");
    pub fn clear(mut self) -> Loading<'a> {
        self.clear = true;
        self // chainable
    }
    pub fn chars(mut self, chars: &'a str) -> Loading<'a> {
        self.chars = chars;
        self // chainable
    }
    pub fn template(mut self, template: &'a str) -> Loading<'a> {
        self.template = template;
        self // chainable
    }

    pub fn start(&mut self) {
        self.timer = Instant::now(); // reset timer

        let delay = self.delay;
        let clear = self.clear;
        let bar = ProgressBar::new(100).with_style(
            ProgressStyle::default_bar()
                .progress_chars(&self.chars)
                .template(&self.template),
        );

        let (tx, rx): (Sender<()>, Receiver<()>) = mpsc::channel();
        self.thread = Some(tx);

        thread::spawn(move || loop {
            // Allow termination of progress bar thread by parent Loading instance:
            match rx.try_recv() {
                Ok(_) | Err(TryRecvError::Disconnected) => {
                    if clear {
                        bar.finish_and_clear();
                    } else {
                        bar.finish();
                    }
                    break;
                }
                _ => (),
            }
            bar.inc(1);
            if bar.position() >= 100 {
                if clear {
                    bar.finish_and_clear();
                } else {
                    bar.finish();
                }
                break;
            }
            thread::sleep(delay);
        });
    }

    pub fn finish(&mut self) {
        self.eta = self.timer.elapsed();
        self.delay = Duration::from_millis(((self.eta.as_millis() as f64) / 100.0).round() as u64);
        if let Some(tx) = &self.thread {
            // Finish progress bar and terminate thread if still running:
            let _ = tx.send(());
        }
    }
}

pub struct Monitor {
    rx: Receiver<Event>,
    watcher: FsEventWatcher,
    paths: Vec<PathBuf>,
}

impl Monitor {
    pub fn new(debounce_ms: u64) -> Result<Monitor> {
        // Create a channel to communicate with Notify watcher:
        let (tx, rx) = mpsc::channel();
        let debounce = Duration::from_millis(debounce_ms);
        let watcher = notify::watcher(tx, debounce)?;
        Ok(Monitor {
            rx,
            watcher,
            paths: vec![],
        })
    }

    // Use Builder Pattern to configure Monitor instance
    // e.g. Monitor::new(1_000).path("~/notes").watch(|event_result| match event_result { ... );
    pub fn path<P: AsRef<Path>>(mut self, path: P) -> Monitor {
        // Generic P: accept PathBuf or &str (and convert to PathBuf)
        self.paths.push([path].iter().collect());
        self
    }

    pub fn watch<F: Fn(Result<Event>)>(mut self, handler: F) -> Result<()> {
        // Watch for file changes in target directory via Notify:
        // https://docs.rs/notify/4.0.10/notify

        // Add paths to be watched:
        // (all files/dirs at that path and below will be monitored)
        for path in self.paths.iter() {
            self.watcher.watch(path, RecursiveMode::Recursive)?;
        }

        loop {
            let event_result = self.rx.recv();

            if let Ok(ref event) = event_result {
                if let Event::Create(ref path) = event {
                    // New file created -- ensure it's watched:
                    self.watcher.watch(path, RecursiveMode::Recursive)?;
                }
            }

            match event_result {
                Ok(event) => handler(Ok(event)),
                Err(error) => handler(Err(error.into())),
                // convert into boxed error
            };
        }
    }
}

// Use strum to allow Converter enum to map to conversion CLI commands:
// https://docs.rs/strum/0.20.0/strum/
#[derive(strum_macros::Display)]
enum Converter {
    #[strum(serialize = "pandoc")]
    MarkdownToHtml,
    #[strum(serialize = "asciidoctor")]
    AsciidocToHtml,
    #[strum(serialize = "wkhtmltopdf")]
    HtmlToPdf,
    #[strum(serialize = "wkhtmltoimage")]
    HtmlToPng,
}

pub fn handle_write(
    path: &PathBuf,
    loading: &mut Loading,
    display: bool,
    verbose: bool,
    quiet: bool,
) -> Result<()> {
    if let Some(ext) = path.extension() {
        match ext.to_str() {
            Some("md") => {
                loading.start();

                let mut outhtml = path.clone();
                outhtml.set_extension("html");

                handle_proc(
                    convert(
                        Converter::MarkdownToHtml,
                        &path,
                        &outhtml,
                        display,
                        verbose,
                        quiet,
                    )?,
                    Converter::MarkdownToHtml,
                    verbose,
                );
            }
            Some("adoc") => {
                loading.start();

                let mut outhtml = path.clone();
                outhtml.set_extension("html");

                handle_proc(
                    convert(
                        Converter::AsciidocToHtml,
                        &path,
                        &outhtml,
                        display,
                        verbose,
                        quiet,
                    )?,
                    Converter::AsciidocToHtml,
                    verbose,
                );
            }
            Some("html") => {
                let mut outpdf = path.clone();
                outpdf.set_extension("pdf");

                let proc_pdf = convert(
                    Converter::HtmlToPdf,
                    &path,
                    &outpdf,
                    display,
                    verbose,
                    quiet,
                )?;

                let mut outpng = path.clone();
                outpng.set_extension("png");

                let proc_png = convert(
                    Converter::HtmlToPng,
                    &path,
                    &outpng,
                    display,
                    verbose,
                    quiet,
                )?;

                if !quiet && !display {
                    // don't overwrite png-convert log with loading bar:
                    println!("");
                }

                handle_proc(proc_pdf, Converter::HtmlToPdf, verbose);
                handle_proc(proc_png, Converter::HtmlToPng, verbose);
            }
            Some("png") => {
                if !quiet {
                    loading.finish();

                    if display {
                        Command::new("imgcat").arg(&path).spawn()?;
                    }
                }
            }
            _ => (),
        }
    }
    Ok(())
}

fn handle_proc(proc: Child, converter: Converter, verbose: bool) {
    if verbose {
        if let Some(stdout) = proc.stdout {
            BufReader::new(stdout).lines().for_each(|line| {
                println!(". . . {} [stdout] . . .", converter.to_string());
                println!(
                    "{}",
                    line.unwrap_or_else(|_| format!(
                        "error: failed to process stdout for {}",
                        converter.to_string()
                    ))
                );
            });
        }

        if let Some(stderr) = proc.stderr {
            BufReader::new(stderr).lines().for_each(|line| {
                println!(". . . {} [stderr] . . .", converter.to_string());
                println!(
                    "{}",
                    line.unwrap_or_else(|_| format!(
                        "error: failed to process stderr for {}",
                        converter.to_string()
                    ))
                );
            });
        }
    }
}

fn convert(
    converter: Converter,
    input: &PathBuf,
    output: &PathBuf,
    display: bool,
    verbose: bool,
    quiet: bool,
) -> Result<Child> {
    if !quiet && !display {
        println!("{} -> {}", input.display(), output.display());
    }

    let mut command = Command::new(converter.to_string());

    command.stdout(Stdio::piped()).stderr(Stdio::piped());

    match converter {
        Converter::MarkdownToHtml | Converter::AsciidocToHtml => {
            if verbose {
                command.arg("--verbose");
            }
        }
        Converter::HtmlToPdf | Converter::HtmlToPng => {
            if !verbose {
                command.arg("--log-level").arg("none");
            }
        }
    }

    let proc: Child;

    match converter {
        Converter::MarkdownToHtml => {
            proc = command.arg(input).arg("-o").arg(output).spawn()?;
        }
        Converter::AsciidocToHtml => {
            proc = command.arg(input).spawn()?;
        }
        Converter::HtmlToPdf | Converter::HtmlToPng => {
            proc = command.arg(input).arg(output).spawn()?;
        }
    }

    Ok(proc)
}
