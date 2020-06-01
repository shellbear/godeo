use std::fs::File;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tempfile::{NamedTempFile, TempPath};
use url::Url;

use crate::context::{error::ContextError, ContextResult};

pub enum Input {
    Path(PathBuf),
    URL(Url),
}

pub enum InputPath {
    Path(PathBuf),
    TempPath(TempPath),
}

impl Input {
    pub fn path(&self) -> ContextResult<InputPath> {
        match self {
            Input::Path(p) => Ok(InputPath::Path(p.to_path_buf())),
            Input::URL(url) => {
                let mut resp = reqwest::blocking::get(url.as_str()).map_err(ContextError::Http)?;
                let size = bytesize::ByteSize(resp.content_length().unwrap_or(0));
                if let Some(content_type) = resp.headers().get("content-type") {
                    let content_type = content_type
                        .to_str()
                        .map_err(|err| ContextError::Error(Box::new(err)))?;

                    match content_type.splitn(2, '/').collect::<Vec<_>>()[..] {
                        ["video", parts] => println!("Video: {:?}", parts),
                        ["audio", parts] => println!("Audio: {:?}", parts),
                        _ => {
                            return Err(ContextError::Error(
                                format!("Invalid Content-type: {:?}", content_type).into(),
                            ))
                        }
                    };
                }

                println!("URL detected");
                println!("Media size: {:?}", size);
                println!("Starting download...");
                let start = Instant::now();

                let tmp = NamedTempFile::new().map_err(ContextError::Io)?;
                let mut file = File::create(tmp.path()).map_err(ContextError::Io)?;

                std::io::copy(&mut resp, &mut file).map_err(ContextError::Io)?;

                println!(
                    "Download finished in {:?} at: {:?}",
                    start.elapsed(),
                    tmp.path()
                );

                Ok(InputPath::TempPath(tmp.into_temp_path()))
            }
        }
    }
}
