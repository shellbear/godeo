mod context;

use crate::context::error::ContextError;
use clap::{App, Arg};
use context::input::Input;
use context::{Context, ContextResult};
use url::Url;

fn main() -> ContextResult {
    let matches = App::new("godeo")
        .version(env!("CARGO_PKG_VERSION"))
        .arg(
            Arg::with_name("input")
                .value_name("INPUT")
                .required(true)
                .index(1),
        )
        .get_matches();

    let path = matches.value_of("input").expect("Missing input argument");
    let input = match path.parse::<Url>() {
        Ok(url) => Input::URL(url),
        Err(_) => Input::Path(path.into()),
    };

    ffmpeg::init().map_err(ContextError::FFmpeg)?;

    let mut context = Context::new(&input)?;

    context
        .add_task(context::Task {
            width: 1280,
            height: 720,
            encoder: "libx264",
            format: "mp4",
            output_file: "x264.mp4",
        })
        .add_task(context::Task {
            width: 1920,
            height: 1080,
            encoder: "libx265",
            format: "mp4",
            output_file: "x265.mp4",
        })
        .run()
}
