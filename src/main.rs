use std::cmp::Ordering;
use std::process::Command;

use clap::Parser;
use command_error::CommandExt;
use command_error::OutputContext;
use miette::miette;
use miette::IntoDiagnostic;
use owo_colors::OwoColorize;
use owo_colors::Style;
use utf8_command::Utf8Output;

mod format_bulleted_list;
mod install_tracing;

use format_bulleted_list::format_bulleted_list;
use install_tracing::install_tracing;

/// A shortcut for `git push --set-upstream REMOTE BRANCH`.
#[derive(Debug, Clone, Parser)]
#[command(version, author, about)]
#[command(max_term_width = 100, disable_help_subcommand = true)]
pub struct Opts {
    /// Log filter directives, of the form `target[span{field=value}]=level`, where all components
    /// except the level are optional.
    ///
    /// Try `debug` or `trace`.
    #[arg(long, default_value = "info", env = "GIT_UPSTREAM_LOG")]
    log: String,

    /// By default, if pushing to a remote fails (e.g. because you don't have permissions),
    /// `git-upstream` will try the next remote until one works.
    ///
    /// With this option, a single failure to push will abort the run.
    #[arg(long)]
    fail_fast: bool,

    /// The branch to push. Defaults to the current branch.
    #[arg(long)]
    branch: Option<String>,

    /// The remote to push to first. Defaults to `origin` if it exists.
    #[arg(env = "GIT_UPSTREAM_REMOTE")]
    remote: Option<String>,
}

impl Opts {
    fn branch(&self) -> miette::Result<String> {
        Ok(match &self.branch {
            Some(branch) => branch.to_owned(),
            None => Command::new("git")
                .args(["rev-parse", "--abbrev-ref", "HEAD"])
                .output_checked_utf8()
                .into_diagnostic()?
                .stdout
                .trim()
                .to_owned(),
        })
    }

    fn remotes(&self) -> miette::Result<Vec<String>> {
        let mut remotes = Command::new("git")
            .args(["remote"])
            .output_checked_as(|context: OutputContext<Utf8Output>| {
                if !context.status().success() {
                    Err(context.error())
                } else {
                    let remotes = context
                        .output()
                        .stdout
                        .lines()
                        .map(|line| line.trim().to_owned())
                        .collect::<Vec<_>>();
                    if remotes.is_empty() {
                        Err(context.error_msg("No Git remotes found"))
                    } else {
                        Ok(remotes)
                    }
                }
            })
            .into_diagnostic()?;

        remotes.sort_by(|a, _b| {
            if a == "origin" {
                Ordering::Less
            } else {
                Ordering::Equal
            }
        });

        if let Some(remote) = &self.remote {
            remotes.sort_by(|a, _b| {
                if a == remote {
                    Ordering::Less
                } else {
                    Ordering::Equal
                }
            });

            if !remotes.contains(remote) {
                let message = format!(
                    "Remote {remote:?} not found. Available Git remotes:\n{}",
                    format_bulleted_list(&remotes)
                );
                if self.fail_fast {
                    // If we only want to try one remote, this is a hard error.
                    return Err(miette!("{message}"));
                } else {
                    // Otherwise, we can try other remotes, so we'll just warn.
                    tracing::warn!("{message}");
                }
            }
        }

        Ok(remotes)
    }
}

fn main() -> miette::Result<()> {
    let opts = Opts::parse();
    install_tracing(&opts.log)?;

    let branch = opts.branch()?;
    let remotes = opts.remotes()?;

    for remote in &remotes {
        tracing::info!(
            "{}",
            format!("$ git push --set-upstream {branch} {remote}").if_supports_color(
                owo_colors::Stream::Stderr,
                |text| Style::new().bold().underline().style(text)
            )
        );

        let result = Command::new("git")
            .args(["push", "--set-upstream", &remote, &branch])
            .status_checked();

        match result {
            Ok(_) => {
                return Ok(());
            }
            Err(err) => {
                if opts.fail_fast {
                    return Err(err).into_diagnostic();
                }
            }
        }
    }

    Err(miette!(
        "Failed to push to all remotes:\n{}",
        format_bulleted_list(&remotes)
    ))
}
