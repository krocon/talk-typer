//! Update check on request only (button in the settings): asks the GitHub API for the releases of
//! the repository and compares the newest one with the running version. Nothing is downloaded or
//! installed – the user gets a link to the release page and installs it themselves.

use crate::error::{AppError, AppResult};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::time::Duration;

const RELEASES_API: &str = "https://api.github.com/repos/krocon/talk-typer/releases?per_page=30";
/// Only pages below this URL may be opened from the frontend.
pub const RELEASES_PAGE: &str = "https://github.com/krocon/talk-typer/releases";

#[derive(Debug, Deserialize)]
pub struct Release {
    pub tag_name: String,
    pub html_url: String,
    #[serde(default)]
    pub draft: bool,
}

/// Result of the check (`UpdateInfo` in TypeScript).
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub current: String,
    /// Newest published version (without `v`), `None` when there is no release yet.
    pub latest: Option<String>,
    /// Release page of `latest`.
    pub url: Option<String>,
    /// `latest` is newer than `current`.
    pub available: bool,
}

fn parse_tag(tag: &str) -> Option<Version> {
    Version::parse(tag.trim().trim_start_matches(['v', 'V'])).ok()
}

/// Newest release that applies to `current`: drafts and unparsable tags are skipped, and
/// pre-release versions (`1.2.0-beta.1`) only count when `current` is a pre-release itself.
pub fn evaluate(current: &Version, releases: &[Release]) -> UpdateInfo {
    let newest = releases
        .iter()
        .filter(|r| !r.draft)
        .filter_map(|r| parse_tag(&r.tag_name).map(|v| (v, r)))
        .filter(|(v, _)| v.pre.is_empty() || !current.pre.is_empty())
        .max_by(|(a, _), (b, _)| a.cmp(b));
    UpdateInfo {
        current: current.to_string(),
        available: newest.as_ref().is_some_and(|(v, _)| v > current),
        latest: newest.as_ref().map(|(v, _)| v.to_string()),
        url: newest.map(|(_, r)| r.html_url.clone()),
    }
}

pub async fn check(current: &Version) -> AppResult<UpdateInfo> {
    let fail = |e: reqwest::Error| AppError::UpdateCheck(e.to_string());
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(20))
        .user_agent(concat!("TalkTyper/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(fail)?;
    let bytes = client
        .get(RELEASES_API)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(fail)?
        .bytes()
        .await
        .map_err(fail)?;
    let releases: Vec<Release> =
        serde_json::from_slice(&bytes).map_err(|e| AppError::UpdateCheck(e.to_string()))?;
    Ok(evaluate(current, &releases))
}

/// `true` for the releases overview and pages below it – nothing else may be opened.
pub fn is_release_url(url: &str) -> bool {
    url == RELEASES_PAGE
        || url.strip_prefix(RELEASES_PAGE).is_some_and(|rest| rest.starts_with('/'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rel(tag: &str, draft: bool) -> Release {
        Release { tag_name: tag.into(), html_url: format!("{RELEASES_PAGE}/tag/{tag}"), draft }
    }

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    #[test]
    fn newer_release_is_found() {
        let info = evaluate(
            &v("0.1.0"),
            &[rel("v0.1.0", false), rel("v0.3.0", false), rel("v0.2.1", false)],
        );
        assert!(info.available);
        assert_eq!(info.latest.as_deref(), Some("0.3.0"));
        assert_eq!(
            info.url.as_deref(),
            Some("https://github.com/krocon/talk-typer/releases/tag/v0.3.0")
        );
    }

    #[test]
    fn same_or_older_is_no_update() {
        let info = evaluate(&v("0.3.0"), &[rel("v0.3.0", false), rel("v0.2.0", false)]);
        assert!(!info.available);
        assert_eq!(info.latest.as_deref(), Some("0.3.0"));
    }

    #[test]
    fn drafts_and_junk_tags_are_ignored() {
        let info = evaluate(&v("0.1.0"), &[rel("v0.9.0", true), rel("nightly", false)]);
        assert_eq!(
            info,
            UpdateInfo { current: "0.1.0".into(), latest: None, url: None, available: false }
        );
    }

    #[test]
    fn pre_releases_only_for_pre_release_users() {
        let releases = [rel("v1.0.0", false), rel("v1.1.0-beta.1", false)];
        assert_eq!(evaluate(&v("1.0.0"), &releases).latest.as_deref(), Some("1.0.0"));
        assert!(evaluate(&v("1.1.0-alpha.1"), &releases).available);
    }

    #[test]
    fn only_release_pages_may_be_opened() {
        assert!(is_release_url(RELEASES_PAGE));
        assert!(is_release_url("https://github.com/krocon/talk-typer/releases/tag/v0.2.0"));
        assert!(!is_release_url("https://github.com/krocon/talk-typer/releases.evil.com/x"));
        assert!(!is_release_url("https://example.com"));
        assert!(!is_release_url("file:///etc/passwd"));
    }
}
