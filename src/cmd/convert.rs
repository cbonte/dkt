use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::Args as ClapArgs;

use crate::cmd::fmt::{self, FmtOptions};
use crate::format::Format;

/// Options of `dkt convert`.
#[derive(ClapArgs, Debug)]
pub struct Args {
    /// File to convert; reads stdin when omitted, or for "-".
    file: Option<PathBuf>,

    /// How the document is written: markdown, haproxy or rst. It comes from the extension of the
    /// file, and it is markdown for the standard input.
    #[arg(long, value_name = "FORMAT")]
    format: Option<String>,

    /// Write to this file instead of stdout.
    #[arg(short, long, value_name = "FILE")]
    output: Option<PathBuf>,

    /// Wrap the prose at this width, as `dkt fmt` does; 0 puts each paragraph on one line. Without
    /// it, the text is written as the conversion gives it.
    #[arg(short, long, value_name = "N")]
    width: Option<usize>,
}

fn read_input(path: Option<&Path>) -> Result<String> {
    let mut text = String::new();
    match path {
        Some(p) if p.as_os_str() != "-" => {
            text = std::fs::read_to_string(p)
                .with_context(|| format!("cannot read {}", p.display()))?;
        }
        _ => {
            io::stdin()
                .read_to_string(&mut text)
                .context("cannot read stdin")?;
        }
    }
    Ok(text)
}

/// The Markdown of a document, wrapped when a width is given.
pub fn convert(args: &Args, text: &str) -> Result<String> {
    let name = args
        .file
        .as_deref()
        .filter(|p| p.as_os_str() != "-")
        .map(|p| p.display().to_string());
    let format = match (&args.format, &name) {
        (None, None) => Format::Markdown,
        (named, Some(name)) => Format::of(named.as_deref(), name)?,
        (Some(named), None) => Format::of(Some(named), "the input")?,
    };
    let dir = args
        .file
        .as_deref()
        .filter(|p| p.as_os_str() != "-")
        .and_then(|p| p.parent());
    let markdown = format.markdown_in(text, dir);
    Ok(match args.width {
        Some(width) => fmt::format(&markdown, &FmtOptions::with_width(width)),
        None => markdown,
    })
}

pub fn run(args: &Args) -> Result<ExitCode> {
    let text = read_input(args.file.as_deref())?;
    let markdown = convert(args, &text)?;
    match &args.output {
        Some(path) => std::fs::write(path, markdown)
            .with_context(|| format!("cannot write {}", path.display()))?,
        None => io::stdout().write_all(markdown.as_bytes())?,
    }
    Ok(ExitCode::SUCCESS)
}
