use std::env;
use std::error;
use std::fmt;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

use indicatif::ProgressBar;

// Standard "error-boxing" Result type.
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
            "options: target='{}' display={} verbose={} quiet={}",
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

// Use strum to allow Converter enum to map to conversion CLI commands:
// https://docs.rs/strum/0.20.0/strum/
#[derive(strum_macros::Display)]
enum Converter {
    #[strum(serialize = "pandoc")]
    Pandoc,
    #[strum(serialize = "asciidoctor")]
    Asciidoctor,
    #[strum(serialize = "wkhtmltopdf")]
    WkHtmlToPdf,
    #[strum(serialize = "wkhtmltoimage")]
    WkHtmlToImage,
}

pub fn handle_write(
    path: &PathBuf,
    loading: &ProgressBar,
    eta: &Duration,
    display: bool,
    verbose: bool,
    quiet: bool,
) -> Result<()> {
    if let Some(ext) = path.extension() {
        match ext.to_str() {
            Some("md") => {
                let mut outhtml = path.clone();
                outhtml.set_extension("html");
                handle_proc(
                    convert(Converter::Pandoc, &path, &outhtml, display, verbose, quiet)?,
                    Converter::Pandoc,
                    loading,
                    eta,
                    verbose,
                    quiet,
                );
            }
            Some("adoc") => {
                let mut outhtml = path.clone();
                outhtml.set_extension("html");
                handle_proc(
                    convert(
                        Converter::Asciidoctor,
                        &path,
                        &outhtml,
                        display,
                        verbose,
                        quiet,
                    )?,
                    Converter::Asciidoctor,
                    loading,
                    eta,
                    verbose,
                    quiet,
                );
            }
            Some("html") => {
                let mut outpdf = path.clone();
                outpdf.set_extension("pdf");
                let proc_pdf = convert(
                    Converter::WkHtmlToPdf,
                    &path,
                    &outpdf,
                    display,
                    verbose,
                    quiet,
                )?;

                let mut outpng = path.clone();
                outpng.set_extension("png");
                let proc_png = convert(
                    Converter::WkHtmlToImage,
                    &path,
                    &outpng,
                    display,
                    verbose,
                    quiet,
                )?;

                if !quiet && !display {
                    // don't overwrite png convert log with loading bar:
                    println!("");
                }

                handle_proc(
                    proc_pdf,
                    Converter::WkHtmlToPdf,
                    loading,
                    eta,
                    verbose,
                    quiet,
                );
                handle_proc(
                    proc_png,
                    Converter::WkHtmlToImage,
                    loading,
                    eta,
                    verbose,
                    quiet,
                );
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

fn handle_proc(
    mut proc: Child,
    converter: Converter,
    loading: &ProgressBar,
    eta: &Duration,
    verbose: bool,
    quiet: bool,
) {
    if !quiet {
        let eta_ms = eta.as_millis();
        let delay = Duration::from_millis(if eta_ms > 0 {
            // 0.8 - adjust by factor based on observation
            ((eta_ms as f64) / 100.0 * 0.8).round() as u64
        } else {
            100
        });
        loop {
            if loading.position() < 100 {
                loading.inc(1);
            }
            if let Ok(Some(_status)) = &proc.try_wait() {
                break;
            }
            sleep(delay);
        }

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
        Converter::Pandoc | Converter::Asciidoctor => {
            if verbose {
                command.arg("--verbose");
            }
        }
        Converter::WkHtmlToPdf | Converter::WkHtmlToImage => {
            if !verbose {
                command.arg("--log-level").arg("none");
            }
        }
    }

    let proc: Child;

    match converter {
        Converter::Pandoc => {
            proc = command.arg(input).arg("-o").arg(output).spawn()?;
        }
        Converter::Asciidoctor => {
            proc = command.arg(input).spawn()?;
        }
        Converter::WkHtmlToPdf | Converter::WkHtmlToImage => {
            proc = command.arg(input).arg(output).spawn()?;
        }
    }

    Ok(proc)
}
