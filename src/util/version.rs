use std::{sync::OnceLock, thread};

use serde::Deserialize;

use crate::ui::{
    message::{Message, UpdateMessage},
    prelude::*,
};

static VERSION_RESULT: OnceLock<Result<CodebergRelease, CheeseError>> = OnceLock::new();

#[derive(Debug, Deserialize, Clone)]
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

/// Process any pending updates, adding the notification if necessary. The outer option is
/// the version check finishing, the inner option is for the message itself
pub fn check_for_updates(update_ignore_version: &str) -> Option<Option<Message>> {
    // We can't do anything if we haven't gotten the response yet
    let version_result = latest()?;

    let release_info = match version_result {
        Ok(release_info) => release_info,
        Err(err) => {
            // Likely either lack of network connection or codeberg being down, we've
            // finished our update check regardless
            log::debug!("Could not fetch version: {err}");
            return Some(None);
        }
    };

    let current_version = semver::Version::parse(current()).unwrap_or_else(|_| {
        log::warn!("we failed to parse our own version, this shouldn't be possible");
        semver::Version::new(0, 0, 0)
    });

    let compare_version = if update_ignore_version.is_empty() {
        // we don't have an ignore version, we only need to compare against the current version
        current_version
    } else {
        match semver::Version::parse(update_ignore_version) {
            Ok(update_ignore_version) => std::cmp::max(update_ignore_version, current_version),
            Err(err) => {
                log::debug!(
                    "Could not parse saved version in data: {}, err: {err}",
                    update_ignore_version
                );
                current_version
            }
        }
    };

    match semver::Version::parse(&release_info.tag_name) {
        Ok(release_tag) => {
            if release_tag > compare_version {
                log::debug!("Found newer release: {release_tag}");
                Some(Some(Message::Update(UpdateMessage::new(
                    release_info.clone(),
                ))))
            } else {
                log::debug!("Latest release is {release_tag}, finished checking updates");
                Some(None)
            }
        }
        Err(err) => {
            log::warn!(
                "We processed a release but were not able to parse it as a semver tag: {release_info:?}: err: {err}"
            );
            Some(None)
        }
    }
}
