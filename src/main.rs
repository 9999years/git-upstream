use std::collections::BTreeSet;
use std::process::Command;

use clap::Parser;
use command_error::CommandExt;
use command_error::OutputContext;
use command_error::Utf8ProgramAndArgs;
use fs_err as fs;
use miette::miette;
use miette::Context;
use miette::IntoDiagnostic;
use owo_colors::OwoColorize;
use owo_colors::Style;
use serde::Deserialize;
use utf8_command::Utf8Output;

mod install_tracing;

use install_tracing::install_tracing;
use xdg::BaseDirectories;

/// Configuration, both from the command-line and a user configuration file.
pub struct Config {
    /// User directories.
    pub dirs: BaseDirectories,
    /// User configuration file.
    pub file: ConfigFile,
    /// Command-line options.
    pub cli: Cli,
}

impl Config {
    pub fn new() -> miette::Result<Self> {
        let dirs = BaseDirectories::with_prefix("git-upstream").into_diagnostic()?;
        let file = {
            let path = dirs.get_config_file("config.toml");
            if !path.exists() {
                ConfigFile::default()
            } else {
                toml::from_str(
                    &fs::read_to_string(path)
                        .into_diagnostic()
                        .wrap_err("Failed to read configuration file")?,
                )
                .into_diagnostic()
                .wrap_err("Failed to deserialize configuration file")?
            }
        };
        let cli = Cli::parse();
        Ok(Self { dirs, file, cli })
    }

    /// Get the remote names to push to, if they exist, highest preferences first.
    pub fn remote_preferences(&self) -> Vec<String> {
        let mut ret = Vec::new();

        if let Some(remote) = &self.cli.remote {
            ret.push(remote.clone());
        }

        if !self.file.remotes.is_empty() {
            ret.extend(self.file.remotes.iter().cloned());
        } else {
            ret.push("origin".into());
        }

        ret
    }

    pub fn list_remotes(&self) -> miette::Result<BTreeSet<String>> {
        Command::new("git")
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
                        .collect::<BTreeSet<_>>();
                    if remotes.is_empty() {
                        Err(context.error_msg("No Git remotes found"))
                    } else {
                        Ok(remotes)
                    }
                }
            })
            .into_diagnostic()
    }

    pub fn branch(&self) -> miette::Result<String> {
        Ok(match &self.cli.branch {
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

    /// Try to push to the given remote.
    ///
    /// If successful, returns `true`.
    pub fn try_push(&self, branch: &str, remote: &str) -> miette::Result<bool> {
        let mut command = Command::new("git");
        command.args(["push", "--set-upstream", remote, branch]);
        if self.cli.force {
            command.arg("--force-with-lease");
        } else if self.cli.force_unchecked {
            command.arg("--force");
        }
        if self.cli.no_verify {
            command.arg("--no-verify");
        }
        command.args(&self.cli.git_push_args);

        let command_display = Utf8ProgramAndArgs::from(&command);

        tracing::info!(
            "{}",
            format!("{command_display}").if_supports_color(owo_colors::Stream::Stderr, |text| {
                Style::new().bold().underline().style(text)
            })
        );

        let result = command.status_checked();

        match result {
            Ok(_) => Ok(true),
            Err(err) => {
                if self.cli.fail_fast {
                    Err(err).into_diagnostic()
                } else {
                    tracing::debug!(%remote, "Failed to push to Git remote");
                    Ok(false)
                }
            }
        }
    }
}

/// Configuration file format.
///
/// TODO: Default configuration file for documentation.
///
/// TODO: Add `fail-fast`/`on-failure` behavior.
#[derive(Deserialize, Default)]
pub struct ConfigFile {
    /// Remotes to attempt to push to, in order.
    #[serde(default)]
    remotes: Vec<String>,
}

/// A shortcut for `git push --set-upstream REMOTE BRANCH`.
#[derive(Debug, Clone, Parser)]
#[command(version, author, about)]
#[command(max_term_width = 100, disable_help_subcommand = true)]
pub struct Cli {
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

    /// Push with `--force-with-lease`.
    #[arg(short, long, alias = "force-with-lease")]
    force: bool,

    /// Push with `--force`.
    #[arg(long)]
    force_unchecked: bool,

    /// Skip pre-push hook.
    #[arg(long)]
    no_verify: bool,

    /// The branch to push. Defaults to the current branch.
    #[arg(long)]
    branch: Option<String>,

    /// The remote to push to first. Defaults to `origin` if it exists and no `remotes` are
    /// set in the configuration file.
    #[arg(env = "GIT_UPSTREAM_REMOTE")]
    remote: Option<String>,

    /// Extra arguments to pass to `git push`.
    #[arg(last = true)]
    git_push_args: Vec<String>,
}

fn main() -> miette::Result<()> {
    let config = Config::new()?;
    install_tracing(&config.cli.log)?;

    let branch = config.branch()?;
    let remote_preferences = config.remote_preferences();
    let mut remotes = config.list_remotes()?;

    for remote in remote_preferences {
        if !remotes.remove(&remote) {
            tracing::debug!(%remote, "Git remote not found");
            continue;
        }

        if config.try_push(&branch, &remote)? {
            return Ok(());
        }
    }

    // Try rest of remotes (not listed on CLI or in config file or `origin`).
    // TODO: Kind of weird to do this alphabetically? Not sure how Git sorts them though...
    for remote in remotes {
        if config.try_push(&branch, &remote)? {
            return Ok(());
        }
    }

    Err(miette!("Failed to upstream {branch} to any remote"))
}
