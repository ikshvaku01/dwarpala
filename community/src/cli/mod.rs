use crate::output::OutputStyle;
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
    #[arg(long, value_enum)]
    pub style: Option<OutputStyle>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_flag_parses() {
        let cli = Cli::try_parse_from(["dwarpala", ".", "--style", "comment"]).unwrap();
        assert_eq!(cli.style, Some(OutputStyle::Comment));
    }

    #[test]
    fn style_flag_is_optional_and_validated() {
        assert_eq!(Cli::try_parse_from(["dwarpala", "."]).unwrap().style, None);
        assert!(Cli::try_parse_from(["dwarpala", ".", "--style", "nope"]).is_err());
    }
}