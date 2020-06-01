use std::boxed::Box;
use std::fmt;

#[derive(Debug)]
pub enum ContextError {
    Io(std::io::Error),
    Http(reqwest::Error),
    FFmpeg(ffmpeg::Error),
    Error(Box<dyn std::error::Error>),
}
