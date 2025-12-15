use std::path::Path;
use std::process::Command;

use anyhow::{anyhow, Context, Result};
use camino::Utf8PathBuf;
use shippo_core::{NodeBinaryConfig, PackagePlan, ProjectType};
use tracing::info;

#[derive(Debug, Clone)]
pub struct BuiltTarget {
    pub target: String,
    pub artifacts: Vec<Utf8PathBuf>,
}

pub fn build_package(
    plan: &PackagePlan,
    workspace_root: &Path,
    version: &str,
    verbose: bool,
) -> Result<Vec<BuiltTarget>> {
    let mut outputs = Vec::new();
    for target in &plan.targets {
        match plan.project_type {
            ProjectType::Rust => outputs.push(build_rust(plan, workspace_root, target, verbose)?),
            ProjectType::Go => {
                outputs.push(build_go(plan, workspace_root, target, verbose, version)?)
            }
            ProjectType::Node => outputs.push(build_node(plan, workspace_root, target, verbose)?),
            ProjectType::Python => {
                outputs.push(build_python(plan, workspace_root, target, verbose)?)
            }
        }
    }
    Ok(outputs)
}

fn build_rust(
    plan: &PackagePlan,
    workspace_root: &Path,
    target: &str,
    verbose: bool,
) -> Result<BuiltTarget> {
    let use_cross = std::env::var("SHIPPO_USE_CROSS").is_ok()
        || (target != "native" && which::which("cross").is_ok());
    let mut cmd = if use_cross && target != "native" {
        let mut c = Command::new("cross");
        c.arg("build").arg("--release").arg("--target").arg(target);
        c
    } else {
        let mut c = Command::new("cargo");
        c.arg("build").arg("--release");
        if target != "native" {
            c.arg("--target").arg(target);
        }
        c
    };
    cmd.current_dir(workspace_root.join(plan.path.as_str()));
    run(cmd, verbose)?;
    let binary_dir = if target == "native" {
        workspace_root
            .join(plan.path.as_str())
            .join("target/release")
    } else {
        workspace_root
            .join(plan.path.as_str())
            .join("target")
            .join(target)
            .join("release")
    };
    let mut artifacts = Vec::new();
    if binary_dir.exists() {
        for entry in std::fs::read_dir(&binary_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && is_executable(&path) {
                if let Ok(p) = Utf8PathBuf::from_path_buf(path) {
                    artifacts.push(p);
                }
            }
        }
    }
    if artifacts.is_empty() {
        return Err(anyhow!("no binaries produced for {}", plan.name));
    }
    Ok(BuiltTarget {
        target: target.to_string(),
        artifacts,
    })
}

fn build_go(
    plan: &PackagePlan,
    workspace_root: &Path,
    target: &str,
    verbose: bool,
    version: &str,
) -> Result<BuiltTarget> {
    let parts: Vec<&str> = target.split(['-', '/']).collect();
    let (goos, goarch) = if parts.len() >= 2 {
        (parts[0], parts[1])
    } else {
        ("", "")
    };
    let mut cmd = Command::new("go");
    cmd.arg("build");
    if !goos.is_empty() {
        cmd.env("GOOS", goos);
    }
    if !goarch.is_empty() {
        cmd.env("GOARCH", goarch);
    }
    cmd.arg("-ldflags")
        .arg(format!("-X main.version={} -X main.commit=", version));
    cmd.current_dir(workspace_root.join(plan.path.as_str()));
    run(cmd, verbose)?;
    let mut artifacts = Vec::new();
    let bin = workspace_root
        .join(plan.path.as_str())
        .join(plan.name.clone());
    if bin.exists() {
        artifacts
            .push(Utf8PathBuf::from_path_buf(bin).map_err(|e| anyhow!(e.display().to_string()))?);
    }
    Ok(BuiltTarget {
        target: target.to_string(),
        artifacts,
    })
}

fn build_node(
    plan: &PackagePlan,
    workspace_root: &Path,
    target: &str,
    verbose: bool,
) -> Result<BuiltTarget> {
    let mut node_cfg = plan.node.clone().unwrap_or_default();
    let project_dir = workspace_root.join(plan.path.as_str());
    let mut npm_ci = Command::new("npm");
    npm_ci.arg("ci").current_dir(&project_dir);
    run(npm_ci, verbose)?;
    if node_cfg.mode == "frontend" {
        if let Some(cmd) = node_cfg.frontend.as_ref().and_then(|f| f.build_cmd.clone()) {
            run(shell_cmd(&cmd, &project_dir), verbose)?;
        } else {
            let mut npm_build = Command::new("npm");
            npm_build.arg("run").arg("build").current_dir(&project_dir);
            run(npm_build, verbose)?;
        }
        let build_dir = node_cfg
            .frontend
            .as_ref()
            .map(|f| f.build_dir.clone())
            .unwrap_or_else(|| "dist".to_string());
        let build_path = project_dir.join(&build_dir);
        if !build_path.exists() {
            return Err(anyhow!(
                "frontend build_dir '{}' not found after build in {}",
                build_dir,
                project_dir.display()
            ));
        }
        let path =
            Utf8PathBuf::from_path_buf(build_path).map_err(|e| anyhow!(e.display().to_string()))?;
        Ok(BuiltTarget {
            target: target.to_string(),
            artifacts: vec![path],
        })
    } else {
        if node_cfg.binary.is_none() {
            node_cfg.binary = Some(NodeBinaryConfig {
                tool: "pkg".into(),
                entry: Some("index.js".into()),
                targets: vec![target.to_string()],
            });
        }
        let bin_cfg = node_cfg
            .binary
            .ok_or_else(|| anyhow!("node.cli-binary requires [node.binary]"))?;
        let entry = bin_cfg.entry.unwrap_or_else(|| "index.js".to_string());
        let mut cmd = Command::new(&bin_cfg.tool);
        cmd.arg(entry);
        if !bin_cfg.targets.is_empty() {
            cmd.arg("--targets").arg(bin_cfg.targets.join(","));
        }
        cmd.current_dir(&project_dir);
        run(cmd, verbose)?;
        let mut artifacts = Vec::new();
        for entry in std::fs::read_dir(&project_dir)? {
            let entry = entry?;
            if entry.file_name().to_string_lossy().contains(&plan.name) {
                if let Ok(p) = Utf8PathBuf::from_path_buf(entry.path()) {
                    artifacts.push(p);
                }
            }
        }
        if artifacts.is_empty() {
            return Err(anyhow!("node binary build produced no outputs"));
        }
        Ok(BuiltTarget {
            target: target.to_string(),
            artifacts,
        })
    }
}

fn build_python(
    plan: &PackagePlan,
    workspace_root: &Path,
    target: &str,
    verbose: bool,
) -> Result<BuiltTarget> {
    let py_cfg = plan.python.clone().unwrap_or_default();
    let project_dir = workspace_root.join(plan.path.as_str());
    if py_cfg.mode == "pyinstaller" {
        let mut cmd = Command::new("pyinstaller");
        let entry = py_cfg
            .pyinstaller
            .as_ref()
            .and_then(|p| p.entry.clone())
            .unwrap_or_else(|| "main.py".to_string());
        cmd.arg("--noconfirm");
        if let Some(pi) = py_cfg.pyinstaller.as_ref() {
            if pi.mode == "onefile" {
                cmd.arg("--onefile");
            }
            for hidden in &pi.hidden_imports {
                cmd.arg("--hidden-import").arg(hidden);
            }
        }
        cmd.arg(entry);
        cmd.current_dir(&project_dir);
        run(cmd, verbose)?;
        let mut artifacts = Vec::new();
        let dist_dir = project_dir.join("dist");
        if dist_dir.exists() {
            for entry in std::fs::read_dir(dist_dir)? {
                let entry = entry?;
                if let Ok(p) = Utf8PathBuf::from_path_buf(entry.path()) {
                    artifacts.push(p);
                }
            }
        }
        Ok(BuiltTarget {
            target: target.to_string(),
            artifacts,
        })
    } else {
        let mut py_build = Command::new("python");
        py_build.args(["-m", "build"]).current_dir(&project_dir);
        run(py_build, verbose)?;
        let mut artifacts = Vec::new();
        let dist_dir = project_dir.join("dist");
        if dist_dir.exists() {
            for entry in std::fs::read_dir(dist_dir)? {
                let entry = entry?;
                if let Ok(p) = Utf8PathBuf::from_path_buf(entry.path()) {
                    artifacts.push(p);
                }
            }
        }
        Ok(BuiltTarget {
            target: target.to_string(),
            artifacts,
        })
    }
}

fn run(mut cmd: Command, verbose: bool) -> Result<()> {
    let printable = format!("{:?}", cmd);
    if verbose {
        info!("running" = ?cmd);
    }
    let status = cmd
        .status()
        .with_context(|| format!("failed to spawn command {printable}"))?;
    if !status.success() {
        return Err(anyhow!("command {printable} failed with status {status}"));
    }
    Ok(())
}

fn shell_cmd(cmd: &str, dir: &Path) -> Command {
    let mut command = if cfg!(target_os = "windows") {
        let mut c = Command::new("cmd");
        c.args(["/C", cmd]);
        c
    } else {
        let mut c = Command::new("sh");
        c.args(["-c", cmd]);
        c
    };
    command.current_dir(dir);
    command
}

fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let meta = std::fs::metadata(path).ok();
        meta.map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(windows)]
    {
        path.extension().map(|e| e == "exe").unwrap_or(false)
    }
}
