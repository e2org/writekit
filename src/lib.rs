use std::env;
use std::error;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

// Standard "error-boxing" Result type.
type Result<T> = ::std::result::Result<T, Box<dyn error::Error>>;

#[derive(Debug)]
pub struct Options {
    pub target: PathBuf,
    pub verbose: bool,
    pub quiet: bool,
}

impl Options {
    pub fn new(matches: clap::ArgMatches) -> Options {
        let verbose = matches.is_present("verbose");
        let quiet = matches.is_present("quiet");

        // Read "target" argument, a file or directory to be watched for changes.
        // If none provided, set to current directory (in which script was invoked).
        let target = match matches.value_of("TARGET") {
            Some(arg) => [arg].iter().collect(), // build PathBuf from arg string
            None => env::current_dir().unwrap(), // use current directory
        };

        Options {
            target,
            verbose,
            quiet,
        }
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

pub fn handle_write(path: PathBuf, verbose: bool, quiet: bool) -> Result<()> {
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

    let proc: Child;

    match converter {
        Converter::Pandoc => {
            proc = command
                .arg(if verbose { "--verbose" } else { "" })
                .arg(input)
                .arg("-o")
                .arg(output)
                .spawn()?;
        }
        Converter::Asciidoctor => {
            proc = command
                .arg(if verbose { "--verbose" } else { "" })
                .arg(input)
                .spawn()?;
        }
        Converter::WkHtmlToPdf | Converter::WkHtmlToImage => {
            proc = command
                .arg("--log-level")
                .arg(if verbose { "info" } else { "none" })
                .arg(input)
                .arg(output)
                .spawn()?;
        }
    }

    Ok(proc)
}

fn print_stdout_stderr(converter: Converter, proc: Child) {
    BufReader::new(proc.stdout.unwrap())
        .lines()
        .for_each(|line| {
            println!("...{}[stdout]...", converter.to_string());
            println!("{}", line.unwrap());
        });

    BufReader::new(proc.stderr.unwrap())
        .lines()
        .for_each(|line| {
            println!("...{}[stderr]...", converter.to_string());
            println!("{}", line.unwrap());
        });
}
