use std::fs;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use reqwest::blocking::Client;
use reqwest::header::{ACCEPT, AUTHORIZATION, USER_AGENT};
use serde::Serialize;
use shippo_core::Manifest;
use shippo_git::{changelog_between, latest_tag};

#[derive(Debug, Clone)]
pub struct ReleaseInput<'a> {
    pub owner: &'a str,
    pub repo: &'a str,
    pub tag: &'a str,
    pub name: &'a str,
    pub draft: bool,
    pub prerelease: bool,
    pub changelog_mode: &'a str,
    pub dist: &'a Path,
    pub manifest: &'a Manifest,
}

#[derive(Serialize)]
struct CreateRelease<'a> {
    tag_name: &'a str,
    name: &'a str,
    body: &'a str,
    draft: bool,
    prerelease: bool,
}

pub fn publish_github(token: &str, input: &ReleaseInput) -> Result<()> {
    let client = Client::new();
    let body = changelog_body(input.changelog_mode, input.tag)?;
    let url = format!(
        "https://api.github.com/repos/{}/{}/releases",
        input.owner, input.repo
    );
    let payload = CreateRelease {
        tag_name: input.tag,
        name: input.name,
        body: &body,
        draft: input.draft,
        prerelease: input.prerelease,
    };
    let res = client
        .post(&url)
        .header(USER_AGENT, "shippo/1.0")
        .header(ACCEPT, "application/vnd.github+json")
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .json(&payload)
        .send()
        .context("failed to create release")?;
    if !res.status().is_success() {
        return Err(anyhow!("github release creation failed: {}", res.status()));
    }
    let release: serde_json::Value = res.json().context("release json parse")?;
    let upload_url = release
        .get("upload_url")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("missing upload_url"))?
        .replace("{?name,label}", "");
    upload_artifacts(token, &upload_url, input)?;
    Ok(())
}

fn upload_artifacts(token: &str, upload_url: &str, input: &ReleaseInput) -> Result<()> {
    let client = Client::new();
    for entry in std::fs::read_dir(input.dist)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let url = format!("{}?name={}", upload_url, name);
        let data = fs::read(&path)?;
        let res = client
            .post(&url)
            .header(USER_AGENT, "shippo/1.0")
            .header(ACCEPT, "application/vnd.github+json")
            .header(AUTHORIZATION, format!("Bearer {}", token))
            .body(data)
            .send()?;
        if !res.status().is_success() {
            return Err(anyhow!("failed to upload {}: {}", name, res.status()));
        }
    }
    Ok(())
}

fn changelog_body(mode: &str, tag: &str) -> Result<String> {
    let prev = latest_tag().unwrap_or_else(|| "".to_string());
    if prev.is_empty() {
        return Ok(format!("Release {}", tag));
    }
    Ok(changelog_between(&prev, tag, mode).unwrap_or_else(|_| format!("Release {}", tag)))
}
