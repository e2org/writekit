use std::env;
use std::error;
use std::fmt;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

// Standard "error-boxing" Result type.
type Result<T> = ::std::result::Result<T, Box<dyn error::Error>>;

pub struct Args {
    pub target: PathBuf,
    pub verbose: bool,
    pub quiet: bool,
}

impl fmt::Display for Args {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "options: target='{}' verbose={} quiet={}",
            self.target.display(),
            self.verbose,
            self.quiet
        )
    }
}

impl Args {
    pub fn new(matches: clap::ArgMatches) -> Result<Args> {
        let verbose = matches.is_present("verbose");
        let quiet = matches.is_present("quiet");

        // Read "target" argument, a file or directory to be watched for changes.
        // If none provided, set to current directory (in which script was invoked).
        let target = match matches.value_of("TARGET") {
            Some(arg) => [arg].iter().collect(), // build PathBuf from arg string
            None => env::current_dir()?,         // use current directory if availalbe
        };

        Ok(Args {
            target,
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

pub fn handle_write(path: &PathBuf, verbose: bool, quiet: bool) -> Result<()> {
    if let Some(ext) = path.extension() {
        match ext.to_str() {
            Some("md") => {
                let mut outhtml = path.clone();
                outhtml.set_extension("html");
                let proc = convert(Converter::Pandoc, &path, &outhtml, verbose, quiet)?;

                if verbose {
                    print_stdout_stderr(Converter::WkHtmlToPdf, proc);
                }
            }
            Some("adoc") => {
                let mut outhtml = path.clone();
                outhtml.set_extension("html");
                let proc = convert(Converter::Asciidoctor, &path, &outhtml, verbose, quiet)?;

                if verbose {
                    print_stdout_stderr(Converter::WkHtmlToPdf, proc);
                }
            }
            Some("html") => {
                let mut outpdf = path.clone();
                outpdf.set_extension("pdf");
                let proc_pdf = convert(Converter::WkHtmlToPdf, &path, &outpdf, verbose, quiet)?;

                let mut outpng = path.clone();
                outpng.set_extension("png");
                let proc_png = convert(Converter::WkHtmlToImage, &path, &outpng, verbose, quiet)?;

                if verbose {
                    print_stdout_stderr(Converter::WkHtmlToPdf, proc_pdf);
                    print_stdout_stderr(Converter::WkHtmlToImage, proc_png);
                }
            }
            _ => (),
        }
    }
    Ok(())
}

fn convert(
    converter: Converter,
    input: &PathBuf,
    output: &PathBuf,
    verbose: bool,
    quiet: bool,
) -> Result<Child> {
    if !quiet {
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

fn print_stdout_stderr(converter: Converter, proc: Child) {
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
