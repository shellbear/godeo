use crate::context::error::ContextError;
use crate::context::ContextResult;
use reqwest::blocking::Client;
use reqwest::Method;
use url::Url;

pub enum Hook {
    Web(Url, Method),
}

impl Hook {
    pub fn execute(&self) -> ContextResult<bool> {
        match self {
            Hook::Web(url, method) => {
                let client = Client::builder().build().map_err(ContextError::Http)?;
                let resp = client
                    .request(method.clone(), url.clone())
                    .send()
                    .map_err(ContextError::Http)?;

                Ok(resp.status().is_success())
            }
        }
    }
}
