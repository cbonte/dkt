use std::collections::BTreeMap;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use clap::Args as ClapArgs;

use crate::format::Format;
use crate::{page, template};

/// Options of `dkt html`.
#[derive(ClapArgs, Debug)]
pub struct Args {
    /// File to render; reads stdin when omitted, or for "-".
    file: Option<PathBuf>,

    /// How the document is written: markdown, haproxy or rst. It comes from the extension of the
    /// file, and it is markdown for the standard input.
    #[arg(long, value_name = "FORMAT")]
    format: Option<String>,

    /// Jinja template that sets the page; without it only the body is written.
    #[arg(long, value_name = "FILE")]
    template: Option<PathBuf>,

    /// A variable for the template, read there as `vars.KEY`. May be repeated.
    #[arg(long = "set", value_name = "KEY=VALUE", value_parser = parse_pair)]
    set: Vec<(String, String)>,

    /// Write to this file instead of stdout.
    #[arg(short, long, value_name = "FILE")]
    output: Option<PathBuf>,
}

fn parse_pair(arg: &str) -> Result<(String, String), String> {
    arg.split_once('=')
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .ok_or_else(|| format!("'{arg}' is not KEY=VALUE"))
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

/// Renders the document, in the template when there is one.
pub fn render(args: &Args, text: &str) -> Result<String> {
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
    let markdown = format.markdown(text);
    let page = page::render(&markdown, None);
    let Some(template) = &args.template else {
        if !args.set.is_empty() {
            bail!("--set needs --template");
        }
        return Ok(page.content);
    };
    let vars: BTreeMap<String, String> = args.set.iter().cloned().collect();
    let path = args
        .file
        .as_deref()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    template::render(template, &page, &vars, &path)
}

pub fn run(args: &Args) -> Result<ExitCode> {
    let text = read_input(args.file.as_deref())?;
    let html = render(args, &text)?;
    match &args.output {
        Some(path) => std::fs::write(path, html)
            .with_context(|| format!("cannot write {}", path.display()))?,
        None => io::stdout().write_all(html.as_bytes())?,
    }
    Ok(ExitCode::SUCCESS)
}
