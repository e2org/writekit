use std::env;
use std::error;
use std::ffi::OsStr;
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

pub fn handle_write(path: PathBuf, verbose: bool, quiet: bool) -> Result<()> {
    match path.extension() {
        Some(ext) => match ext.to_str() {
            Some("adoc") => adoc_to_html(&path, verbose, quiet)?,
            Some("html") => {
                // TODO parallelize
                html_to_pdf(&path, verbose, quiet)?;
                html_to_png(&path, verbose, quiet)?;
            }
            _ => (),
        },
        None => (),
    };
    Ok(())
}

fn adoc_to_html(in_path: &PathBuf, verbose: bool, quiet: bool) -> Result<()> {
    if !quiet {
        let mut out_path = in_path.clone();

        out_path.set_extension("html");

        // TODO better error handling instead of unwrap below?
        println!(
            "{} -> {}",
            in_path.file_name().unwrap().to_string_lossy(),
            out_path.file_name().unwrap().to_string_lossy(),
        );
    }

    let output = Command::new("asciidoctor").arg(in_path).output()?;

    if verbose {
        println!("{}", String::from_utf8(output.stdout)?);
    }

    Ok(())
}

fn html_to_pdf(in_path: &PathBuf, verbose: bool, quiet: bool) -> Result<()> {
    let mut out_path = in_path.clone();

    out_path.set_extension("pdf");

    if !quiet {
        // TODO better error handling instead of unwrap below?
        println!(
            "{} -> {}",
            in_path.file_name().unwrap().to_string_lossy(),
            out_path.file_name().unwrap().to_string_lossy(),
        );
    }

    let output = Command::new("wkhtmltopdf")
        .arg(in_path)
        .arg(out_path)
        .output()?;

    if verbose {
        println!("{}", String::from_utf8(output.stdout)?);
    }

    Ok(())
}

fn html_to_png(in_path: &PathBuf, verbose: bool, quiet: bool) -> Result<()> {
    let mut out_path = in_path.clone();

    out_path.set_extension("png");

    if !quiet {
        // TODO better error handling instead of unwrap below?
        println!(
            "{} -> {}",
            in_path.file_name().unwrap().to_string_lossy(),
            out_path.file_name().unwrap().to_string_lossy(),
        );
    }

    let output = Command::new("wkhtmltoimage")
        .arg(in_path)
        .arg(out_path)
        .output()?;

    if verbose {
        println!("{}", String::from_utf8(output.stdout)?);
    }

    Ok(())
}
