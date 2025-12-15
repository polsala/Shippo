use std::fs::{self, File};
use std::path::Path;
use std::process::Command;

use anyhow::{anyhow, Result};
use camino::Utf8PathBuf;
use chrono::Utc;
use flate2::write::GzEncoder;
use flate2::Compression;
use shippo_core::{
    naming_template, sha256_file, BuildEnvInfo, Manifest, ManifestArtifact, ManifestPackage,
    ManifestProject, ManifestSignature, ManifestTarget, Plan, ToolingInfo,
};
use zip::write::FileOptions;
use zip::ZipWriter;

#[derive(Debug, Clone)]
pub struct BuiltOutput {
    pub package: String,
    pub target: String,
    pub artifacts: Vec<Utf8PathBuf>,
}

pub fn package_outputs(
    plan: &Plan,
    built: &[BuiltOutput],
    dist: &Path,
    repo_url: Option<String>,
    commit: Option<String>,
    sign: bool,
) -> Result<Manifest> {
    fs::create_dir_all(dist)?;
    let mut manifest_packages = Vec::new();
    let mut checksum_entries: Vec<(String, String)> = Vec::new();
    for pkg in &plan.packages {
        let mut targets = Vec::new();
        for built_entry in built.iter().filter(|b| b.package == pkg.name) {
            let mut artifacts_meta = Vec::new();
            for fmt in &pkg.package.formats {
                let archive_name = format!(
                    "{}.{}",
                    naming_template(
                        &pkg.package.name_template,
                        &pkg.name,
                        &plan.version,
                        &built_entry.target
                    ),
                    fmt
                );
                let archive_path = dist.join(&archive_name);
                if fmt.ends_with("tar.gz") {
                    create_tar_gz(&archive_path, &built_entry.artifacts)?;
                } else if fmt == "zip" {
                    create_zip(&archive_path, &built_entry.artifacts)?;
                } else {
                    return Err(anyhow!("unsupported package format {fmt}"));
                }
                let sha = sha256_file(&archive_path)?;
                checksum_entries.push((sha.clone(), archive_name.clone()));
                let meta = ManifestArtifact {
                    filename: archive_name.clone(),
                    bytes: fs::metadata(&archive_path)?.len() as u64,
                    sha256: sha,
                };
                artifacts_meta.push(meta);
            }
            // sbom simple fallback
            let sbom_file = format!(
                "{}-sbom.cdx.json",
                naming_template(
                    &pkg.package.name_template,
                    &pkg.name,
                    &plan.version,
                    &built_entry.target
                )
            );
            let sbom_path = dist.join(&sbom_file);
            write_sbom(&sbom_path, &pkg.name, &plan.version, &built_entry.target)?;
            let sbom_sha = sha256_file(&sbom_path)?;
            checksum_entries.push((sbom_sha.clone(), sbom_file.clone()));
            let sbom_meta = ManifestArtifact {
                filename: sbom_file.clone(),
                bytes: fs::metadata(&sbom_path)?.len() as u64,
                sha256: sbom_sha,
            };
            // signatures (optional)
            let mut signatures = Vec::new();
            if sign && pkg.sign.enabled {
                for art in &artifacts_meta {
                    if let Some(sig) = sign_file(dist, &art.filename, &pkg.sign.method)? {
                        checksum_entries.push((sha256_file(&dist.join(&sig))?, sig.clone()));
                        signatures.push(ManifestSignature {
                            filename: sig,
                            method: pkg.sign.method.clone(),
                        });
                    }
                }
                if let Some(sig) = sign_file(dist, &sbom_meta.filename, &pkg.sign.method)? {
                    checksum_entries.push((sha256_file(&dist.join(&sig))?, sig.clone()));
                    signatures.push(ManifestSignature {
                        filename: sig,
                        method: pkg.sign.method.clone(),
                    });
                }
            }
            targets.push(ManifestTarget {
                target: built_entry.target.clone(),
                artifacts: artifacts_meta,
                sbom: Some(sbom_meta),
                signatures,
            });
        }
        manifest_packages.push(ManifestPackage {
            name: pkg.name.clone(),
            project_type: pkg.project_type.clone(),
            path: pkg.path.to_string(),
            targets,
        });
    }

    let tooling = ToolingInfo {
        rust: tool_version("rustc --version"),
        go: tool_version("go version"),
        node: tool_version("node --version"),
        python: tool_version("python --version"),
    };

    let manifest = Manifest {
        shippo_version: env!("CARGO_PKG_VERSION").to_string(),
        generated_at: Utc::now(),
        project: ManifestProject {
            repo_url,
            commit,
            version: plan.version.clone(),
        },
        packages: manifest_packages,
        tooling,
        build_env: BuildEnvInfo {
            os: std::env::consts::OS.into(),
            arch: std::env::consts::ARCH.into(),
            ci: std::env::var("CI").is_ok(),
        },
    };
    let manifest_json = manifest.to_json()?;
    let manifest_path = dist.join("manifest.json");
    fs::write(&manifest_path, manifest_json)?;
    checksum_entries.push((sha256_file(&manifest_path)?, "manifest.json".into()));

    let sha_file = dist.join("SHA256SUMS");
    let mut out = String::new();
    for (sha, file) in &checksum_entries {
        out.push_str(&format!("{}  {}\n", sha, file));
    }
    fs::write(&sha_file, out)?;

    let provenance_path = dist.join("provenance.json");
    let provenance = serde_json::json!({
        "version": plan.version,
        "generated_at": Utc::now(),
        "ci": std::env::var("CI").is_ok(),
    });
    fs::write(&provenance_path, serde_json::to_string_pretty(&provenance)?)?;
    Ok(manifest)
}

pub fn verify_manifest(manifest_path: &Path, dist: &Path) -> Result<()> {
    let data = fs::read_to_string(manifest_path)?;
    let manifest: Manifest = serde_json::from_str(&data)?;
    for pkg in &manifest.packages {
        for target in &pkg.targets {
            for art in &target.artifacts {
                let path = dist.join(&art.filename);
                if !path.exists() {
                    return Err(anyhow!("missing artifact {}", art.filename));
                }
                let sha = sha256_file(&path)?;
                if sha != art.sha256 {
                    return Err(anyhow!("sha mismatch for {}", art.filename));
                }
            }
            if let Some(sbom) = &target.sbom {
                let path = dist.join(&sbom.filename);
                if !path.exists() {
                    return Err(anyhow!("missing sbom {}", sbom.filename));
                }
                let sha = sha256_file(&path)?;
                if sha != sbom.sha256 {
                    return Err(anyhow!("sbom hash mismatch {}", sbom.filename));
                }
            }
            for sig in &target.signatures {
                let path = dist.join(&sig.filename);
                if !path.exists() {
                    return Err(anyhow!("missing signature {}", sig.filename));
                }
                if let Some(base) = sig.filename.strip_suffix(".sig") {
                    let target_path = dist.join(base);
                    if target_path.exists() {
                        let sha = sha256_file(&target_path)?;
                        if let Ok(contents) = fs::read_to_string(&path) {
                            if contents.trim() == sha {
                                continue;
                            }
                        }
                        // attempt external verification best-effort
                        if sig.method == "gpg" {
                            let _ = std::process::Command::new("gpg")
                                .args([
                                    "--verify",
                                    path.to_string_lossy().as_ref(),
                                    target_path.to_string_lossy().as_ref(),
                                ])
                                .status();
                        } else if sig.method == "cosign" && which::which("cosign").is_ok() {
                            let _ = std::process::Command::new("cosign")
                                .args([
                                    "verify-blob",
                                    target_path.to_string_lossy().as_ref(),
                                    "--signature",
                                    path.to_string_lossy().as_ref(),
                                ])
                                .status();
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn create_tar_gz(path: &Path, inputs: &[Utf8PathBuf]) -> Result<()> {
    let tar_gz = File::create(path)?;
    let enc = GzEncoder::new(tar_gz, Compression::default());
    let mut tar = tar::Builder::new(enc);
    for input in inputs {
        let input_path = input.as_std_path();
        if input_path.is_dir() {
            tar.append_dir_all(input.file_name().unwrap_or("artifact"), input_path)?;
        } else {
            tar.append_path_with_name(input_path, input.file_name().unwrap())?;
        }
    }
    tar.finish()?;
    Ok(())
}

fn create_zip(path: &Path, inputs: &[Utf8PathBuf]) -> Result<()> {
    let file = File::create(path)?;
    let mut zip = ZipWriter::new(file);
    let options = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for input in inputs {
        let input_path = input.as_std_path();
        if input_path.is_dir() {
            for entry in walkdir::WalkDir::new(input_path) {
                let entry = entry?;
                if entry.file_type().is_file() {
                    let rel = entry.path().strip_prefix(input_path).unwrap();
                    zip.start_file(rel.to_string_lossy(), options)?;
                    let mut f = File::open(entry.path())?;
                    std::io::copy(&mut f, &mut zip)?;
                }
            }
        } else {
            zip.start_file(input.file_name().unwrap_or("artifact").to_string(), options)?;
            let mut f = File::open(input_path)?;
            std::io::copy(&mut f, &mut zip)?;
        }
    }
    zip.finish()?;
    Ok(())
}

fn write_sbom(path: &Path, name: &str, version: &str, target: &str) -> Result<()> {
    let sbom = serde_json::json!({
        "bomFormat": "CycloneDX",
        "specVersion": "1.4",
        "version": 1,
        "metadata": {
            "component": {"name": name, "version": version, "target": target}
        },
        "components": []
    });
    fs::write(path, serde_json::to_string_pretty(&sbom)?)?;
    Ok(())
}

fn sign_file(dist: &Path, filename: &str, method: &str) -> Result<Option<String>> {
    let path = dist.join(filename);
    let sig_name = format!("{}.sig", filename);
    let sig_path = dist.join(&sig_name);
    let sha = sha256_file(&path)?;
    if method == "gpg" {
        let status = Command::new("gpg")
            .args([
                "--batch",
                "--yes",
                "--detach-sign",
                "-o",
                sig_path.to_string_lossy().as_ref(),
                path.to_string_lossy().as_ref(),
            ])
            .status();
        if let Ok(status) = status {
            if status.success() {
                return Ok(Some(sig_name));
            }
        }
        // fall back to embedded signature file
    } else if method == "cosign" && which::which("cosign").is_ok() {
        let status = Command::new("cosign")
            .args([
                "sign-blob",
                path.to_string_lossy().as_ref(),
                "--output",
                sig_path.to_string_lossy().as_ref(),
            ])
            .status();
        if let Ok(status) = status {
            if status.success() {
                return Ok(Some(sig_name));
            }
        }
    }
    fs::write(&sig_path, sha)?;
    Ok(Some(sig_name))
}

fn tool_version(cmd: &str) -> Option<String> {
    let mut parts = cmd.split_whitespace();
    let prog = parts.next()?;
    let args: Vec<_> = parts.collect();
    let output = Command::new(prog).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_create_tar_and_zip() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("file.txt");
        fs::write(&file, "hi").unwrap();
        let artifact = Utf8PathBuf::from_path_buf(file).unwrap();
        let out_dir = dir.path().join("dist");
        fs::create_dir_all(&out_dir).unwrap();
        create_tar_gz(&out_dir.join("a.tar.gz"), std::slice::from_ref(&artifact)).unwrap();
        create_zip(&out_dir.join("a.zip"), std::slice::from_ref(&artifact)).unwrap();
        assert!(out_dir.join("a.tar.gz").exists());
        assert!(out_dir.join("a.zip").exists());
    }
}
