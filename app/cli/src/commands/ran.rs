//! `suzatary ran <file>`: a model run's journal line, its counts and ids read off the CLI's
//! `result` object, never its words, which quote the mail the run read.

use std::path::{Path, PathBuf};

use anyhow::Context as _;
use suzatary_io::model::summary::{Outcome, summary_of};

/// What `suzatary ran` was told.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// The run's output, as `claude -p --output-format json` or `stream-json --verbose`
    /// printed it.
    output: PathBuf,
    /// The prefix of the tools the run had to call (`mcp__suzatary__`); with it, the run's
    /// verdict is the exit code: 10 the CLI is not signed in, 11 it called none of them, 12 it
    /// printed no result, 13 its result is not a shape this box reads. Above the codes the
    /// shell and a failed verb take for their own.
    #[arg(long)]
    needs: Option<String>,
}

/// Why a run that had to call its tools did not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Failed {
    SignedOut = 10,
    CalledNone = 11,
    NoResult = 12,
    Unreadable = 13,
}

/// Print the run's summary line; a file that holds no result object says only how long it is.
///
/// # Errors
/// The file unreadable.
pub fn run(args: &Args) -> anyhow::Result<()> {
    let text = std::fs::read_to_string(&args.output)
        .with_context(|| format!("reading {}", args.output.display()))?;
    let summary = summary_of(&text);
    match &summary {
        Ok(Some(summary)) => println!("{summary}"),
        Err(unreadable) => println!("{unreadable}"),
        Ok(None) => println!(
            "the run printed no result object ({} bytes, kept at {})",
            text.len(),
            Path::new(&args.output).display()
        ),
    }
    let Some(needs) = &args.needs else {
        return Ok(());
    };
    let failed = match &summary {
        Err(_) => Some(Failed::Unreadable),
        Ok(None) => Some(Failed::NoResult),
        Ok(Some(summary)) if summary.outcome == Outcome::SignedOut => Some(Failed::SignedOut),
        Ok(Some(summary)) if !summary.called.keys().any(|name| name.starts_with(needs)) => {
            Some(Failed::CalledNone)
        }
        Ok(Some(_)) => None,
    };
    if let Some(failed) = failed {
        std::process::exit(failed as i32);
    }
    Ok(())
}
