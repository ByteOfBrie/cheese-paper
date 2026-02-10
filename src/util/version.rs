use std::{sync::OnceLock, thread};

use crate::ui::prelude::*;

static VERSION_RESULT: OnceLock<Result<String, CheeseError>> = OnceLock::new();

pub fn current() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub fn latest() -> Option<&'static str> {
    VERSION_RESULT
        .get()
        .and_then(|res| res.as_ref().ok().map(|s| s.as_str()))
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
fn get_version() -> Result<String, CheeseError> {
    let body: String =
        ureq::get("https://codeberg.org/api/v1/repos/byteofbrie/cheese-paper/releases/latest")
            .header("TODO-Header", "todo value")
            .call()
            .map_err(|err| cheese_error!("Error resolving request: {err}"))?
            .body_mut()
            .read_to_string()
            .map_err(|err| cheese_error!("Error accessing request content: {err}"))?;

    // TODO extract version from request body

    Ok(body)
}
