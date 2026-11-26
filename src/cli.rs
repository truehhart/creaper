use clap::{Parser, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ListKind {
    Languages,
    Categories,
}

#[derive(Debug, Parser)]
#[command(
    name = "creaper",
    version,
    about = "Strict multi-language comment and docstring linter"
)]
pub struct Cli {
    #[arg(value_name = "PATH")]
    pub paths: Vec<PathBuf>,

    #[arg(short, long, value_name = "FILE")]
    pub config: Option<PathBuf>,

    #[arg(long, conflicts_with = "config")]
    pub no_config: bool,

    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    pub format: OutputFormat,

    #[arg(long)]
    pub tracked: bool,

    #[arg(long, value_enum, value_name = "KIND")]
    pub list: Option<ListKind>,
}
