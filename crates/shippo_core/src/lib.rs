use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{anyhow, Result};
use camino::{Utf8Path, Utf8PathBuf};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

pub static DEFAULT_CONFIG: &str =
    "# Shippo configuration\n[project]\nname = \"example\"\ntype = \"rust\"\npath = \".\"\n\n[version]\nsource = \"git\"\n\n[build]\ntargets = [\"native\"]\n\n[package]\nformats = [\"tar.gz\", \"zip\"]\nname_template = \"{name}-{version}-{target}\"\n\n[sbom]\nenabled = true\nformat = \"cyclonedx\"\nmode = \"auto\"\n\n[sign]\nenabled = false\nmethod = \"cosign\"\ncosign_mode = \"keyless\"\n\n[release]\nprovider = \"github\"\ndraft = true\nprerelease = false\n\n[release.github]\nowner = \"acme\"\nrepo = \"example\"\n\n[changelog]\nmode = \"auto\"\n";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProjectType {
    Rust,
    Go,
    Node,
    Python,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectConfig {
    pub name: String,
    #[serde(rename = "type")]
    pub project_type: ProjectType,
    #[serde(default = "default_dot")]
    pub path: String,
}

fn default_dot() -> String {
    ".".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum VersionSource {
    Tag,
    Manual,
    Git,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VersionConfig {
    pub source: VersionSource,
    #[serde(default)]
    pub manual: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BuildConfig {
    #[serde(default = "default_targets")]
    pub targets: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

fn default_targets() -> Vec<String> {
    vec!["native".to_string()]
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PackageConfig {
    #[serde(default = "default_formats")]
    pub formats: Vec<String>,
    #[serde(default = "default_template")]
    pub name_template: String,
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
}

fn default_formats() -> Vec<String> {
    vec!["tar.gz".to_string(), "zip".to_string()]
}

fn default_template() -> String {
    "{name}-{version}-{target}".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SbomConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_sbom_format")]
    pub format: String,
    #[serde(default = "default_sbom_mode")]
    pub mode: String,
}

fn default_true() -> bool {
    true
}

fn default_sbom_format() -> String {
    "cyclonedx".to_string()
}

fn default_sbom_mode() -> String {
    "auto".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignConfig {
    #[serde(default = "default_false")]
    pub enabled: bool,
    #[serde(default = "default_sign_method")]
    pub method: String,
    #[serde(default = "default_cosign_mode")]
    pub cosign_mode: String,
}

fn default_false() -> bool {
    false
}

fn default_sign_method() -> String {
    "cosign".to_string()
}

fn default_cosign_mode() -> String {
    "keyless".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReleaseConfig {
    #[serde(default = "default_release_provider")]
    pub provider: String,
    #[serde(default = "default_true")]
    pub draft: bool,
    #[serde(default = "default_false")]
    pub prerelease: bool,
    #[serde(default)]
    pub github: Option<GitHubReleaseConfig>,
}

fn default_release_provider() -> String {
    "github".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GitHubReleaseConfig {
    pub owner: String,
    pub repo: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChangelogConfig {
    #[serde(default = "default_changelog_mode")]
    pub mode: String,
    #[serde(default)]
    pub file: Option<String>,
}

fn default_changelog_mode() -> String {
    "auto".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeBinaryConfig {
    #[serde(default = "default_node_tool")]
    pub tool: String,
    pub entry: Option<String>,
    #[serde(default)]
    pub targets: Vec<String>,
}

fn default_node_tool() -> String {
    "pkg".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeFrontendConfig {
    #[serde(default = "default_frontend_dir")]
    pub build_dir: String,
    #[serde(default)]
    pub build_cmd: Option<String>,
}

fn default_frontend_dir() -> String {
    "dist".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeConfig {
    #[serde(default = "default_node_mode")]
    pub mode: String,
    #[serde(default)]
    pub binary: Option<NodeBinaryConfig>,
    #[serde(default)]
    pub frontend: Option<NodeFrontendConfig>,
}

fn default_node_mode() -> String {
    "cli-binary".to_string()
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            mode: default_node_mode(),
            binary: None,
            frontend: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PyInstallerConfig {
    #[serde(default = "default_py_mode")]
    pub mode: String,
    pub entry: Option<String>,
    #[serde(default)]
    pub hidden_imports: Vec<String>,
    #[serde(default)]
    pub data: Vec<String>,
}

fn default_py_mode() -> String {
    "onefile".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PythonConfig {
    #[serde(default = "default_python_mode")]
    pub mode: String,
    #[serde(default)]
    pub pyinstaller: Option<PyInstallerConfig>,
}

fn default_python_mode() -> String {
    "wheel".to_string()
}

impl Default for PythonConfig {
    fn default() -> Self {
        Self {
            mode: default_python_mode(),
            pyinstaller: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PackageEntry {
    pub name: String,
    #[serde(rename = "type")]
    pub project_type: ProjectType,
    #[serde(default = "default_dot")]
    pub path: String,
    #[serde(default)]
    pub build: Option<BuildConfig>,
    #[serde(default)]
    pub package: Option<PackageConfig>,
    #[serde(default)]
    pub sbom: Option<SbomConfig>,
    #[serde(default)]
    pub sign: Option<SignConfig>,
    #[serde(default)]
    pub node: Option<NodeConfig>,
    #[serde(default)]
    pub python: Option<PythonConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShippoConfig {
    #[serde(default)]
    pub project: Option<ProjectConfig>,
    #[serde(default)]
    pub packages: Vec<PackageEntry>,
    #[serde(default)]
    pub node: Option<NodeConfig>,
    #[serde(default)]
    pub python: Option<PythonConfig>,
    #[serde(default)]
    pub version: Option<VersionConfig>,
    #[serde(default)]
    pub build: Option<BuildConfig>,
    #[serde(default)]
    pub package: Option<PackageConfig>,
    #[serde(default)]
    pub sbom: Option<SbomConfig>,
    #[serde(default)]
    pub sign: Option<SignConfig>,
    #[serde(default)]
    pub release: Option<ReleaseConfig>,
    #[serde(default)]
    pub changelog: Option<ChangelogConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PackagePlan {
    pub name: String,
    pub project_type: ProjectType,
    pub path: Utf8PathBuf,
    pub targets: Vec<String>,
    pub package: PackageConfig,
    pub sbom: SbomConfig,
    pub sign: SignConfig,
    pub node: Option<NodeConfig>,
    pub python: Option<PythonConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Plan {
    pub version: String,
    pub packages: Vec<PackagePlan>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManifestArtifact {
    pub filename: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManifestSignature {
    pub filename: String,
    #[serde(default)]
    pub method: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManifestTarget {
    pub target: String,
    pub artifacts: Vec<ManifestArtifact>,
    pub sbom: Option<ManifestArtifact>,
    pub signatures: Vec<ManifestSignature>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManifestPackage {
    pub name: String,
    #[serde(rename = "type")]
    pub project_type: ProjectType,
    pub path: String,
    pub targets: Vec<ManifestTarget>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManifestProject {
    pub repo_url: Option<String>,
    pub commit: Option<String>,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolingInfo {
    pub rust: Option<String>,
    pub go: Option<String>,
    pub node: Option<String>,
    pub python: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BuildEnvInfo {
    pub os: String,
    pub arch: String,
    pub ci: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Manifest {
    pub shippo_version: String,
    pub generated_at: DateTime<Utc>,
    pub project: ManifestProject,
    pub packages: Vec<ManifestPackage>,
    pub tooling: ToolingInfo,
    pub build_env: BuildEnvInfo,
}

impl Manifest {
    pub fn to_json(&self) -> Result<String> {
        let mut value = serde_json::to_value(self)?;
        if let Some(obj) = value.as_object_mut() {
            // ensure deterministic order by sorting keys
            let mut sorted = serde_json::Map::new();
            let mut keys: Vec<_> = obj.keys().cloned().collect();
            keys.sort();
            for k in keys {
                sorted.insert(k.clone(), obj.remove(&k).unwrap());
            }
            value = serde_json::Value::Object(sorted);
        }
        Ok(serde_json::to_string_pretty(&value)?)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ConfigError {
    #[error("configuration error: {0}")]
    Message(String),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub fn load_config(path: &Path) -> Result<ShippoConfig, ConfigError> {
    let content = fs::read_to_string(path).map_err(|e| {
        ConfigError::Message(format!("failed to read config {}: {e}", path.display()))
    })?;
    let mut cfg: ShippoConfig = toml::from_str(&content).map_err(|e| {
        ConfigError::Message(format!("failed to parse toml {}: {e}", path.display()))
    })?;
    validate_config(&mut cfg)?;
    Ok(cfg)
}

fn validate_config(cfg: &mut ShippoConfig) -> Result<(), ConfigError> {
    if cfg.project.is_none() && cfg.packages.is_empty() {
        return Err(ConfigError::Message(
            "config must define [project] or [[packages]]".to_string(),
        ));
    }
    if cfg.project.is_some() && !cfg.packages.is_empty() {
        return Err(ConfigError::Message(
            "use either single [project] or [[packages]] monorepo, not both".to_string(),
        ));
    }
    if let Some(version) = &cfg.version {
        if matches!(version.source, VersionSource::Manual) && version.manual.is_none() {
            return Err(ConfigError::Message(
                "version.source=manual requires version.manual".to_string(),
            ));
        }
    }
    for pkg in &cfg.packages {
        validate_package_entry(pkg)?;
    }
    Ok(())
}

fn validate_package_entry(pkg: &PackageEntry) -> Result<(), ConfigError> {
    if pkg.name.trim().is_empty() {
        return Err(ConfigError::Message("package name required".to_string()));
    }
    if !matches!(
        pkg.project_type,
        ProjectType::Rust | ProjectType::Go | ProjectType::Node | ProjectType::Python
    ) {
        return Err(ConfigError::Message(format!(
            "unsupported project type for {}",
            pkg.name
        )));
    }
    if let Some(node) = &pkg.node {
        if node.mode == "cli-binary" && node.binary.is_none() {
            return Err(ConfigError::Message(
                "node.cli-binary requires [node.binary]".to_string(),
            ));
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct VersionInfo {
    pub value: String,
    pub source: VersionSource,
}

pub fn resolve_version(cfg: &ShippoConfig, tag_override: Option<String>) -> Result<VersionInfo> {
    if let Some(tag) = tag_override {
        return Ok(VersionInfo {
            value: tag,
            source: VersionSource::Manual,
        });
    }
    let version_cfg = cfg.version.as_ref().cloned().unwrap_or(VersionConfig {
        source: VersionSource::Git,
        manual: None,
    });
    match version_cfg.source {
        VersionSource::Manual => Ok(VersionInfo {
            value: version_cfg.manual.unwrap_or_else(|| "0.1.0".to_string()),
            source: VersionSource::Manual,
        }),
        VersionSource::Tag => {
            let tag = latest_tag().unwrap_or_else(|| "v0.1.0".to_string());
            Ok(VersionInfo {
                value: tag,
                source: VersionSource::Tag,
            })
        }
        VersionSource::Git => {
            let tag = latest_tag().unwrap_or_else(|| "v0.1.0".to_string());
            Ok(VersionInfo {
                value: tag,
                source: VersionSource::Git,
            })
        }
    }
}

fn latest_tag() -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["describe", "--tags", "--abbrev=0"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let tag = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if tag.is_empty() {
        None
    } else {
        Some(tag)
    }
}

pub fn build_plan(
    cfg: &ShippoConfig,
    only: Option<&str>,
    tag_override: Option<String>,
) -> Result<Plan> {
    let version = resolve_version(cfg, tag_override)?.value;
    let mut packages = Vec::new();
    if let Some(project) = &cfg.project {
        if only.is_some() && only != Some(project.name.as_str()) {
            // skip
        } else {
            packages.push(resolve_package(
                project,
                cfg.build.as_ref(),
                cfg.package.as_ref(),
                cfg.sbom.as_ref(),
                cfg.sign.as_ref(),
                cfg,
            )?);
        }
    }
    for pkg in &cfg.packages {
        if let Some(only_name) = only {
            if only_name != pkg.name {
                continue;
            }
        }
        let build = pkg.build.as_ref().or(cfg.build.as_ref());
        let package = pkg.package.as_ref().or(cfg.package.as_ref());
        let sbom = pkg.sbom.as_ref().or(cfg.sbom.as_ref());
        let sign = pkg.sign.as_ref().or(cfg.sign.as_ref());
        packages.push(resolve_package_entry(
            pkg,
            build,
            package,
            sbom,
            sign,
            cfg.node.as_ref(),
            cfg.python.as_ref(),
        )?);
    }
    if packages.is_empty() {
        return Err(anyhow!("no packages selected"));
    }
    Ok(Plan { version, packages })
}

fn resolve_package(
    project: &ProjectConfig,
    build: Option<&BuildConfig>,
    package: Option<&PackageConfig>,
    sbom: Option<&SbomConfig>,
    sign: Option<&SignConfig>,
    cfg: &ShippoConfig,
) -> Result<PackagePlan> {
    let pkg_entry = PackageEntry {
        name: project.name.clone(),
        project_type: project.project_type.clone(),
        path: project.path.clone(),
        build: build.cloned(),
        package: package.cloned(),
        sbom: sbom.cloned(),
        sign: sign.cloned(),
        node: cfg.node.clone(),
        python: cfg.python.clone(),
    };
    resolve_package_entry(
        &pkg_entry,
        build,
        package,
        sbom,
        sign,
        cfg.node.as_ref(),
        cfg.python.as_ref(),
    )
}

fn resolve_package_entry(
    pkg: &PackageEntry,
    build: Option<&BuildConfig>,
    package: Option<&PackageConfig>,
    sbom: Option<&SbomConfig>,
    sign: Option<&SignConfig>,
    node: Option<&NodeConfig>,
    python: Option<&PythonConfig>,
) -> Result<PackagePlan> {
    let path = Utf8Path::new(&pkg.path).to_owned();
    let targets = build
        .map(|b| b.targets.clone())
        .or_else(|| pkg.build.as_ref().map(|b| b.targets.clone()))
        .unwrap_or_else(default_targets);
    let pkg_cfg = pkg
        .package
        .clone()
        .or_else(|| package.cloned())
        .unwrap_or(PackageConfig {
            formats: default_formats(),
            name_template: default_template(),
            include: Vec::new(),
            exclude: Vec::new(),
        });
    let sbom_cfg = pkg
        .sbom
        .clone()
        .or_else(|| sbom.cloned())
        .unwrap_or(SbomConfig {
            enabled: true,
            format: default_sbom_format(),
            mode: default_sbom_mode(),
        });
    let sign_cfg = pkg
        .sign
        .clone()
        .or_else(|| sign.cloned())
        .unwrap_or(SignConfig {
            enabled: false,
            method: default_sign_method(),
            cosign_mode: default_cosign_mode(),
        });
    Ok(PackagePlan {
        name: pkg.name.clone(),
        project_type: pkg.project_type.clone(),
        path,
        targets,
        package: pkg_cfg,
        sbom: sbom_cfg,
        sign: sign_cfg,
        node: pkg.node.clone().or_else(|| node.cloned()),
        python: pkg.python.clone().or_else(|| python.cloned()),
    })
}

pub fn naming_template(template: &str, name: &str, version: &str, target: &str) -> String {
    template
        .replace("{name}", name)
        .replace("{version}", version)
        .replace("{target}", target)
}

pub fn sha256_file(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher)?;
    Ok(hex::encode(hasher.finalize()))
}

pub fn collect_files(root: &Path, patterns: &[String]) -> Vec<Utf8PathBuf> {
    let mut files = Vec::new();
    for e in WalkDir::new(root).into_iter().flatten() {
        if e.file_type().is_file() {
            let path = e.path();
            if let Ok(p) = Utf8PathBuf::from_path_buf(path.to_path_buf()) {
                if patterns.is_empty() || patterns.iter().any(|pat| p.as_str().contains(pat)) {
                    files.push(p);
                }
            }
        }
    }
    files
}

pub fn detect_projects(root: &Path) -> Vec<ProjectConfig> {
    let mut projects = Vec::new();
    let entries = match fs::read_dir(root) {
        Ok(iter) => iter,
        Err(_) => return projects,
    };
    let mut add_if = |proj: ProjectConfig| {
        if !projects.iter().any(|p| p.name == proj.name) {
            projects.push(proj);
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = entry.file_name().to_string_lossy().to_string();
            let rust = path.join("Cargo.toml");
            let go = path.join("go.mod");
            let pkg_json = path.join("package.json");
            let py = path.join("pyproject.toml");
            if rust.exists() {
                add_if(ProjectConfig {
                    name: name.clone(),
                    project_type: ProjectType::Rust,
                    path: name.clone(),
                });
            }
            if go.exists() {
                add_if(ProjectConfig {
                    name: name.clone(),
                    project_type: ProjectType::Go,
                    path: name.clone(),
                });
            }
            if pkg_json.exists() {
                add_if(ProjectConfig {
                    name: name.clone(),
                    project_type: ProjectType::Node,
                    path: name.clone(),
                });
            }
            if py.exists() {
                add_if(ProjectConfig {
                    name: name.clone(),
                    project_type: ProjectType::Python,
                    path: name.clone(),
                });
            }
        }
    }
    projects
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_naming_template() {
        let out = naming_template("{name}-{version}-{target}", "app", "1.0", "x86");
        assert_eq!(out, "app-1.0-x86");
    }

    #[test]
    fn test_config_validation() {
        let toml =
            "[project]\nname='demo'\ntype='rust'\n\n[version]\nsource='manual'\nmanual='1.2.3'";
        let mut cfg: ShippoConfig = toml::from_str(toml).unwrap();
        validate_config(&mut cfg).unwrap();
    }

    #[test]
    fn test_manifest_json_deterministic() {
        let manifest = Manifest {
            shippo_version: "0.1.0".into(),
            generated_at: Utc::now(),
            project: ManifestProject {
                repo_url: None,
                commit: None,
                version: "v0.1.0".into(),
            },
            packages: vec![],
            tooling: ToolingInfo {
                rust: None,
                go: None,
                node: None,
                python: None,
            },
            build_env: BuildEnvInfo {
                os: "linux".into(),
                arch: "x86_64".into(),
                ci: false,
            },
        };
        let a = manifest.to_json().unwrap();
        let b = manifest.to_json().unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn test_detect_projects() {
        let dir = tempdir().unwrap();
        std::fs::create_dir(dir.path().join("rusty")).unwrap();
        std::fs::write(dir.path().join("rusty/Cargo.toml"), "[package]\nname='r'").unwrap();
        let detected = detect_projects(dir.path());
        assert!(!detected.is_empty());
    }

    #[test]
    fn test_plan_resolution() {
        let toml = "[project]\nname='demo'\ntype='rust'\n\n[build]\ntargets=['native']\n";
        let cfg: ShippoConfig = toml::from_str(toml).unwrap();
        let plan = build_plan(&cfg, None, None).unwrap();
        assert_eq!(plan.packages.len(), 1);
        assert_eq!(plan.packages[0].name, "demo");
    }
}
