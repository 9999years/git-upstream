use std::process::Command;

use clap::Parser;
use command_error::CommandExt;
use miette::IntoDiagnostic;

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

fn main() -> miette::Result<()> {
    let opts = Opts::parse();
    install_tracing(&opts.log)?;

    let branch = match opts.branch {
        Some(branch) => branch,
        None => Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .output_checked_utf8()
            .into_diagnostic()?
            .stdout
            .trim()
            .to_owned(),
    };

    let mut remotes = Vec::new();
    if let Some(remote) = opts.remote {
        remotes.push(remote)
    }

    remotes.extend(
        Command::new("git")
            .args(["remote"])
            .output_checked_utf8()
            .into_diagnostic()?
            .stdout
            .lines()
            .map(|line| line.trim().to_owned()),
    );

    for remote in remotes {
        tracing::info!("$ git push --set-upstream {branch} {remote}");
    }

    Ok(())
}

fn install_tracing(filter_directives: &str) -> miette::Result<()> {
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    use tracing_subscriber::Layer;

    let env_filter = tracing_subscriber::EnvFilter::try_new(filter_directives).into_diagnostic()?;

    let human_layer = tracing_human_layer::HumanLayer::new()
        .with_output_writer(std::io::stderr())
        .with_filter(env_filter);

    let registry = tracing_subscriber::registry();

    registry.with(human_layer).init();

    Ok(())
}
