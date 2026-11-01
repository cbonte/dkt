use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Result;
use clap::Args as ClapArgs;

use crate::site::{self, Options};

/// Options of `dkt site`.
#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Properties file of the site.
    #[arg(default_value = "dkt.toml", value_name = "FILE")]
    config: PathBuf,

    /// Directory the site is written to. It is cleared first, if dkt made it.
    #[arg(short, long, default_value = "public", value_name = "DIR")]
    output: PathBuf,

    /// Address the site is published at, instead of the one of the properties file.
    #[arg(long, value_name = "URL")]
    base_url: Option<String>,

    /// Build without checking the documents first.
    #[arg(long)]
    no_lint: bool,

    /// Build only a part of the site and leave the rest of the directory as it is: `home`, `static`,
    /// `section:NAME` or `page:SRC`. It can be given several times. `versions` alone refreshes the
    /// root of the versions of a documentation from the `[[version]]` tables.
    #[arg(long, value_name = "WHAT")]
    only: Vec<String>,

    /// Build the version NAME of the documentation into DIR/NAME, with the base address followed by
    /// NAME, and leave the rest of DIR as it is.
    #[arg(long, value_name = "NAME")]
    docs_version: Option<String>,
}

pub fn run(args: &Args) -> Result<ExitCode> {
    let report = site::build(&Options {
        config: args.config.clone(),
        out: args.output.clone(),
        base_url: args.base_url.clone(),
        lint: !args.no_lint,
        only: args.only.clone(),
        docs_version: args.docs_version.clone(),
    })?;
    match report.versions {
        Some(count) => eprintln!(
            "dkt site: the root of {count} versions written to {}",
            report.dir.display()
        ),
        None => eprintln!(
            "dkt site: {} pages written to {}",
            report.pages,
            report.dir.display()
        ),
    }
    Ok(ExitCode::SUCCESS)
}
