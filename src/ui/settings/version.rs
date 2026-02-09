use std::rc::Rc;

use crate::{
    cheese_error,
    util::{CheeseError, Promise},
};

#[derive(Debug)]
pub struct Version {
    l: Promise<Result<Rc<String>, CheeseError>>,
}

impl Version {
    pub fn current() -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    pub fn latest(&mut self) -> Option<Rc<String>> {
        self.l.get().and_then(|res| res.as_ref().ok().cloned())
    }

    pub fn new() -> Self {
        let l = Promise::make(async {
            let result = reqwest::get(
                "https://codeberg.org/api/v1/repos/byteofbrie/cheese-paper/releases/latest",
            )
            .await;

            let response = match result {
                Ok(response) => response,
                Err(err) => {
                    let c_err = cheese_error!("Could not resolve remote version: {err}");
                    log::error!("{c_err}");
                    return Err(c_err);
                }
            };

            match response.text().await {
                Ok(text) => {
                    // TODO: correctly parse the result string to get the version number
                    Ok(Rc::new(text))
                }
                Err(err) => {
                    let c_err = cheese_error!("Error waiting for version content: {err}");
                    log::error!("{c_err}");
                    Err(c_err)
                }
            }
        });

        Self { l }
    }
}

impl Default for Version {
    fn default() -> Self {
        Self::new()
    }
}
