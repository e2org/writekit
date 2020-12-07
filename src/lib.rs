use std::env;
use std::error;
use std::fs::File;
use std::path::PathBuf;
use std::process::Command;

use clap::ArgMatches;

type Result<T> = ::std::result::Result<T, Box<dyn error::Error>>;

#[derive(Debug)]
pub struct Options {
    pub target: PathBuf,
    pub verbose: bool,
    pub quiet: bool,
}

impl Options {
    pub fn new(matches: ArgMatches) -> Options {
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

enum Conversion {
    MarkdownToHtml,
    AsciiDocToHtml,
    HtmlToPdf,
    HtmlToPng,
}

pub fn handle_write(path: PathBuf, verbose: bool, quiet: bool) -> Result<()> {
    if let Some(ext) = path.extension() {
        match ext.to_str() {
            Some("md") => {
                let mut outhtml = path.clone();
                outhtml.set_extension("html");
                convert(Conversion::MarkdownToHtml, &path, &outhtml, verbose, quiet)?;
            }
            Some("adoc") => {
                let mut outhtml = path.clone();
                outhtml.set_extension("html");
                convert(Conversion::AsciiDocToHtml, &path, &outhtml, verbose, quiet)?;
            }
            Some("html") => {
                let mut outpdf = path.clone();
                outpdf.set_extension("pdf");
                convert(Conversion::HtmlToPdf, &path, &outpdf, verbose, quiet)?;

                let mut outpng = path.clone();
                outpng.set_extension("png");
                convert(Conversion::HtmlToPng, &path, &outpng, verbose, quiet)?;
            }
            _ => (),
        }
    }
    Ok(())
}

fn convert(
    conversion: Conversion,
    input: &PathBuf,
    output: &PathBuf,
    verbose: bool,
    quiet: bool,
) -> Result<()> {
    if !quiet {
        println!("{} -> {}", input.display(), output.display());
    }

    match conversion {
        Conversion::MarkdownToHtml => {
            Command::new("pandoc")
                .arg(input)
                .arg("-o")
                .arg(output)
                .spawn()?;
        }
        Conversion::AsciiDocToHtml => {
            Command::new("asciidoctor").arg(input).spawn()?;
        }
        Conversion::HtmlToPdf | Conversion::HtmlToPng => {
            Command::new(match conversion {
                Conversion::HtmlToPdf => "wkhtmltopdf",
                _ => "wkhtmltoimage",
            })
            .arg("--log-level")
            .arg(if verbose { "info" } else { "none" })
            .arg(input)
            .arg(output)
            .spawn()?;
        }
    }

    Ok(())
}
