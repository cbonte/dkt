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
    /// `section:NAME` or `page:SRC`. It can be given several times.
    #[arg(long, value_name = "WHAT")]
    only: Vec<String>,
}

pub fn run(args: &Args) -> Result<ExitCode> {
    let report = site::build(&Options {
        config: args.config.clone(),
        out: args.output.clone(),
        base_url: args.base_url.clone(),
        lint: !args.no_lint,
        only: args.only.clone(),
    })?;
    eprintln!(
        "dkt site: {} pages written to {}",
        report.pages,
        args.output.display()
    );
    Ok(ExitCode::SUCCESS)
}
