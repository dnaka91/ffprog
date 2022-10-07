use std::{
    fs::OpenOptions,
    io::{self, Write},
    path::{Path, PathBuf},
};

use anyhow::{ensure, Context, Result};
use clap::{CommandFactory, Parser, Subcommand, ValueHint};
use clap_complete::Shell;

/// Visualizer for the FFmpeg encoding process.
#[derive(Parser)]
#[command(about, author, version, arg_required_else_help(true))]
pub struct Args {
    /// Same input media file that is used in the FFmpeg arguments.
    #[arg(short, long, value_hint  = ValueHint::FilePath)]
    pub input: PathBuf,
    /// Overwrite the output file if it already exists.
    #[arg(short = 'y', long)]
    pub overwrite: bool,
    /// Only load the statistics and display them, skipping any encoding.
    #[arg(short = 's', long)]
    pub load_stats: bool,
    /// Show the statistics screen after the encoding is done.
    #[arg(long)]
    pub show_stats: bool,
    /// Save the statistics to a file, so they can be loaded afterwards.
    #[arg(long)]
    pub save_stats: bool,
    /// Arguments to pass to FFmpeg.
    #[arg(raw = true)]
    pub args: Vec<String>,
    #[command(subcommand)]
    pub cmd: Option<Command>,
}

impl Args {
    pub fn parse() -> Self {
        <Self as Parser>::parse()
    }
}

#[derive(Subcommand)]
pub enum Command {
    /// Generate auto-completion scripts for various shells.
    Completions {
        /// Shell to generate an auto-completion script for.
        #[arg(value_enum)]
        shell: Shell,
    },
    /// Generate man pages into the given directory.
    Manpages {
        /// Target directory, that must already exist and be empty. If the any file with the same
        /// name as any of the man pages already exist, it'll not be overwritten, but instead an
        /// error be returned.
        #[arg(value_hint = ValueHint::DirPath)]
        dir: PathBuf,
    },
}

/// Generate shell completions, written to the standard output.
#[allow(clippy::unnecessary_wraps)]
pub fn completions(shell: Shell) {
    clap_complete::generate(
        shell,
        &mut Args::command(),
        env!("CARGO_PKG_NAME"),
        &mut io::stdout().lock(),
    );
}

/// Generate man pages in the target directory. The directory must already exist and none of the
/// files exist, or an error is returned.
pub fn manpages(dir: &Path) -> Result<()> {
    fn print(dir: &Path, app: &clap::Command) -> Result<()> {
        let name = app.get_display_name().unwrap_or_else(|| app.get_name());
        let out = dir.join(format!("{name}.1"));
        let mut out = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&out)
            .with_context(|| format!("the file `{}` already exists", out.display()))?;

        clap_mangen::Man::new(app.clone()).render(&mut out)?;
        out.flush()?;

        for sub in app.get_subcommands() {
            print(dir, sub)?;
        }

        Ok(())
    }

    ensure!(dir.try_exists()?, "target directory doesn't exist");

    let mut app = Args::command();
    app.build();

    print(dir, &app)
}

#[cfg(test)]
mod tests {
    use super::Args;

    #[test]
    fn verify_cli() {
        use clap::CommandFactory;
        Args::command().debug_assert();
    }
}
