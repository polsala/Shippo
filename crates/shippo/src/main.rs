use std::fs;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use clap::{ArgAction, Parser, Subcommand};
use shippo_core::{
    build_plan, detect_projects, load_config, BuildConfig, PackageEntry, Plan, ShippoConfig,
};
use shippo_git::{current_commit, repo_url};
use shippo_pack::{package_outputs, verify_manifest, BuiltOutput};
use shippo_publish::{publish_github, ReleaseInput};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(
    name = "shippo",
    version,
    author,
    about = "Polyglot release orchestrator"
)]
pub struct Cli {
    /// Path to config file
    #[arg(long, default_value = ".shippo.toml")]
    config: PathBuf,

    /// Only operate on a specific package
    #[arg(long)]
    only: Option<String>,

    /// Verbose logging
    #[arg(long)]
    verbose: bool,

    /// Dry run mode
    #[arg(long)]
    dry_run: bool,

    /// Override version/tag
    #[arg(long, value_name = "TAG")]
    tag: Option<String>,

    /// Force draft release
    #[arg(long, action = ArgAction::SetTrue)]
    draft: bool,

    /// Force non-draft release
    #[arg(long, action = ArgAction::SetTrue)]
    no_draft: bool,

    /// Override prerelease flag
    #[arg(long)]
    prerelease: bool,

    /// Output directory
    #[arg(long, default_value = "dist")]
    output: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Detect projects and generate a default config
    Init,
    /// Show execution plan
    Plan {
        #[arg(long)]
        json: bool,
    },
    /// Build all packages
    Build,
    /// Package artifacts into dist/
    Package,
    /// Build, package and publish release
    Release,
    /// Verify manifest and signatures
    Verify,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    init_logging(cli.verbose);
    match cli.command {
        Commands::Init => cmd_init(&cli),
        Commands::Plan { json } => cmd_plan(&cli, json),
        Commands::Build => cmd_build(&cli, false),
        Commands::Package => cmd_build(&cli, true),
        Commands::Release => cmd_release(&cli),
        Commands::Verify => cmd_verify(&cli),
    }
}

fn init_logging(verbose: bool) {
    let filter = if verbose {
        "shippo=debug"
    } else {
        "shippo=info"
    };
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(filter))
        .with_target(false)
        .try_init();
}

fn load_plan(cli: &Cli) -> Result<Plan> {
    let cfg = load_config(&cli.config).map_err(|e| anyhow!("{e}"))?;
    build_plan(&cfg, cli.only.as_deref(), cli.tag.clone())
        .map_err(|e| anyhow!("failed to build plan: {e}"))
}

fn cmd_init(cli: &Cli) -> Result<()> {
    let path = &cli.config;
    if path.exists() {
        return Err(anyhow!("{} already exists", path.display()));
    }
    let projects = detect_projects(std::path::Path::new("."));
    let mut cfg = ShippoConfig {
        project: None,
        packages: vec![],
        node: None,
        python: None,
        version: None,
        build: Some(BuildConfig {
            targets: vec!["native".into()],
            env: Default::default(),
        }),
        package: None,
        sbom: None,
        sign: None,
        release: None,
        changelog: None,
    };
    if projects.len() == 1 {
        cfg.project = Some(shippo_core::ProjectConfig {
            name: projects[0].name.clone(),
            project_type: projects[0].project_type.clone(),
            path: projects[0].path.clone(),
        });
    } else {
        for proj in projects {
            cfg.packages.push(PackageEntry {
                name: proj.name.clone(),
                project_type: proj.project_type.clone(),
                path: proj.path.clone(),
                build: None,
                package: None,
                sbom: None,
                sign: None,
                node: None,
                python: None,
            });
        }
    }
    let toml = toml::to_string_pretty(&cfg)?;
    fs::write(path, toml)?;
    println!("wrote {}", path.display());
    Ok(())
}

fn cmd_plan(cli: &Cli, json: bool) -> Result<()> {
    let plan = load_plan(cli)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&plan)?);
    } else {
        println!("Plan for version {}", plan.version);
        for pkg in &plan.packages {
            println!(
                "- {} ({:?}) targets: {}",
                pkg.name,
                pkg.project_type,
                pkg.targets.join(", ")
            );
        }
    }
    Ok(())
}

fn cmd_build(cli: &Cli, package_after: bool) -> Result<()> {
    let plan = load_plan(cli)?;
    let mut outputs = Vec::new();
    for pkg in &plan.packages {
        let built = shippo_builders::build_package(
            pkg,
            std::path::Path::new("."),
            &plan.version,
            cli.verbose,
        )?;
        for target in built {
            outputs.push(BuiltOutput {
                package: pkg.name.clone(),
                target: target.target,
                artifacts: target.artifacts,
            });
        }
    }
    if package_after {
        let dist = cli.output.clone();
        let manifest = package_outputs(&plan, &outputs, &dist, repo_url(), current_commit(), true)?;
        println!(
            "packaged {} packages into {}",
            manifest.packages.len(),
            dist.display()
        );
    }
    Ok(())
}

fn cmd_release(cli: &Cli) -> Result<()> {
    let plan = load_plan(cli)?;
    let mut outputs = Vec::new();
    for pkg in &plan.packages {
        let built = shippo_builders::build_package(
            pkg,
            std::path::Path::new("."),
            &plan.version,
            cli.verbose,
        )?;
        for target in built {
            outputs.push(BuiltOutput {
                package: pkg.name.clone(),
                target: target.target,
                artifacts: target.artifacts,
            });
        }
    }
    let dist = cli.output.clone();
    let manifest = package_outputs(&plan, &outputs, &dist, repo_url(), current_commit(), true)?;
    if cli.dry_run {
        println!("dry-run release complete; skipping publish");
        return Ok(());
    }
    let cfg = load_config(&cli.config).map_err(|e| anyhow!("{e}"))?;
    let release_cfg = cfg
        .release
        .ok_or_else(|| anyhow!("release config missing"))?;
    let gh = release_cfg
        .github
        .ok_or_else(|| anyhow!("release.github missing"))?;
    let token = std::env::var("GITHUB_TOKEN").or_else(|_| std::env::var("GH_TOKEN"))?;
    let draft = if cli.no_draft {
        false
    } else if cli.draft {
        true
    } else {
        release_cfg.draft
    };
    let input = ReleaseInput {
        owner: &gh.owner,
        repo: &gh.repo,
        tag: &plan.version,
        name: &plan.version,
        draft,
        prerelease: cli.prerelease || release_cfg.prerelease,
        changelog_mode: &cfg
            .changelog
            .map(|c| c.mode)
            .unwrap_or_else(|| "auto".into()),
        dist: &dist,
        manifest: &manifest,
    };
    publish_github(&token, &input)?;
    println!(
        "published release {} to {}/{}",
        plan.version, gh.owner, gh.repo
    );
    Ok(())
}

fn cmd_verify(cli: &Cli) -> Result<()> {
    let dist = cli.output.clone();
    let manifest_path = dist.join("manifest.json");
    verify_manifest(&manifest_path, &dist)?;
    println!("manifest verified");
    Ok(())
}
