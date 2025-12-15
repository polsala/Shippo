use std::fs;

use camino::Utf8PathBuf;
use shippo_core::{PackageConfig, PackagePlan, Plan, ProjectType, SbomConfig, SignConfig};
use shippo_pack::{package_outputs, verify_manifest, BuiltOutput};
use tempfile::tempdir;

#[test]
fn package_and_verify_manifest() {
    let dir = tempdir().unwrap();
    let artifact_path = dir.path().join("demo-bin");
    fs::write(&artifact_path, "hello").unwrap();
    let artifact = Utf8PathBuf::from_path_buf(artifact_path).unwrap();
    let plan = Plan {
        version: "v1.0.0".into(),
        packages: vec![PackagePlan {
            name: "demo".into(),
            project_type: ProjectType::Rust,
            path: Utf8PathBuf::from("."),
            targets: vec!["native".into()],
            package: PackageConfig {
                formats: vec!["tar.gz".into(), "zip".into()],
                name_template: "{name}-{version}-{target}".into(),
                include: vec![],
                exclude: vec![],
            },
            sbom: SbomConfig {
                enabled: true,
                format: "cyclonedx".into(),
                mode: "auto".into(),
            },
            sign: SignConfig {
                enabled: false,
                method: "cosign".into(),
                cosign_mode: "keyless".into(),
            },
            node: None,
            python: None,
        }],
    };
    let built = vec![BuiltOutput {
        package: "demo".into(),
        target: "native".into(),
        artifacts: vec![artifact],
    }];
    let dist = dir.path().join("dist");
    let manifest = package_outputs(&plan, &built, &dist, None, None, false).unwrap();
    assert_eq!(manifest.packages.len(), 1);
    let manifest_path = dist.join("manifest.json");
    verify_manifest(&manifest_path, &dist).unwrap();
}
