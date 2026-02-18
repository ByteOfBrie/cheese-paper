use std::{sync::OnceLock, thread};

use serde::Deserialize;

use crate::ui::prelude::*;

static VERSION_RESULT: OnceLock<Result<CodebergRelease, CheeseError>> = OnceLock::new();

#[derive(Debug, Deserialize)]
pub struct CodebergRelease {
    pub tag_name: String,
    pub name: String,
    pub body: String,
    pub html_url: String,
}

pub fn current() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub fn latest() -> Option<&'static Result<CodebergRelease, CheeseError>> {
    VERSION_RESULT.get()
}

/// Send a request to fetch the version. Should be called by the app once on startup.
pub fn fetch_version() {
    if VERSION_RESULT.get().is_some() {
        return;
    }

    thread::spawn(move || {
        let version_result = get_version();

        let _ = VERSION_RESULT
            .set(version_result)
            .map_err(|_| log::warn!("Version was fetched multiple times. Ignoring new results"));
    });
}

/// Blocking function which performs an http request
fn get_version() -> Result<CodebergRelease, CheeseError> {
    let body: String =
        ureq::get("https://codeberg.org/api/v1/repos/byteofbrie/cheese-paper/releases/latest")
            .header("accept", "application/json")
            .call()
            .map_err(|err| cheese_error!("Error resolving request: {err}"))?
            .body_mut()
            .read_to_string()
            .map_err(|err| cheese_error!("Error accessing request content: {err}"))?;

    serde_json::from_str::<CodebergRelease>(&body).map_err(|err| {
        log::debug!("got error when attempting to parse release: {err}: {body}");
        cheese_error!("Could not parse release: {err}")
    })
}

/// Really dumb test validating our current version (which cargo also validates anyway), but
/// getting this wrong would break version checking code, so we're extra cautious
#[test]
fn test_valid_current_version() {
    assert!(semver::Version::parse(current()).is_ok());
}
