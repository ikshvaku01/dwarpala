use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "dwarpala", about = "Compile project source files into an AI-ready document")]
pub struct Cli {
    pub root_path: PathBuf,
    #[arg(long, value_delimiter = ',', default_value = "")]
    pub disable: Vec<String>,
    #[arg(long, value_delimiter = ',', default_value = "")]
    pub enable: Vec<String>,
}