use std::path::PathBuf;

use clap::Args;
use sem_core::git::types::DiffScope;

use crate::formatters;
use crate::OutputFormat;
use inspect_core::analyze::{analyze_with_options, retain_entity_reviews, AnalyzeOptions};
use inspect_core::types::RiskLevel;

#[derive(Args)]
pub struct DiffArgs {
    /// Commit ref or range (e.g. HEAD~1, main..feature, abc123)
    #[arg(required_unless_present = "staged", conflicts_with = "staged")]
    pub target: Option<String>,

    /// Analyze changes staged for the next commit
    #[arg(long)]
    pub staged: bool,

    /// Output format
    #[arg(long, value_enum, default_value = "terminal")]
    pub format: OutputFormat,

    /// Minimum risk level to show
    #[arg(long)]
    pub min_risk: Option<String>,

    /// Show dependency context
    #[arg(long)]
    pub context: bool,

    /// Include full source code of dependent entities (callers/consumers)
    #[arg(long)]
    pub dependents: bool,

    /// Repository path
    #[arg(short = 'C', long, default_value = ".")]
    pub repo: PathBuf,
}

pub fn run(args: DiffArgs) {
    let scope = if args.staged {
        DiffScope::Staged
    } else {
        parse_scope(
            args.target
                .as_deref()
                .expect("clap requires a target unless --staged is used"),
        )
    };
    let repo = args.repo.canonicalize().unwrap_or(args.repo.clone());

    let options = AnalyzeOptions {
        include_dependent_code: args.dependents,
        ..AnalyzeOptions::default()
    };

    match analyze_with_options(&repo, scope, &options) {
        Ok(mut result) => {
            // Filter by min risk if specified
            if let Some(ref min) = args.min_risk {
                let min_level = parse_risk_level(min);
                retain_entity_reviews(&mut result, |r| r.risk_level >= min_level);
            }

            match args.format {
                OutputFormat::Terminal => formatters::terminal::print(&result, args.context),
                OutputFormat::Json => formatters::json::print(&result),
                OutputFormat::Markdown => formatters::markdown::print(&result, args.context),
            }
        }
        Err(e) => {
            eprintln!("error: {}", e);
            std::process::exit(1);
        }
    }
}

fn parse_scope(target: &str) -> DiffScope {
    if target.contains("..") {
        let parts: Vec<&str> = target.split("..").collect();
        DiffScope::Range {
            from: parts[0].to_string(),
            to: parts[1].to_string(),
        }
    } else {
        DiffScope::Commit {
            sha: target.to_string(),
        }
    }
}

fn parse_risk_level(s: &str) -> RiskLevel {
    match s.to_lowercase().as_str() {
        "critical" => RiskLevel::Critical,
        "high" => RiskLevel::High,
        "medium" => RiskLevel::Medium,
        _ => RiskLevel::Low,
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::DiffArgs;

    #[derive(Parser)]
    struct TestCli {
        #[command(flatten)]
        diff: DiffArgs,
    }

    #[test]
    fn accepts_staged_without_target() {
        let cli = TestCli::try_parse_from(["inspect-diff", "--staged"]).unwrap();

        assert!(cli.diff.staged);
        assert!(cli.diff.target.is_none());
    }

    #[test]
    fn still_accepts_commit_target() {
        let cli = TestCli::try_parse_from(["inspect-diff", "HEAD~1"]).unwrap();

        assert!(!cli.diff.staged);
        assert_eq!(cli.diff.target.as_deref(), Some("HEAD~1"));
    }

    #[test]
    fn requires_target_or_staged() {
        assert!(TestCli::try_parse_from(["inspect-diff"]).is_err());
    }

    #[test]
    fn rejects_target_with_staged() {
        assert!(TestCli::try_parse_from(["inspect-diff", "HEAD", "--staged"]).is_err());
    }
}
