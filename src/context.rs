pub mod error;
pub mod hook;
pub mod input;

use bus::{Bus, BusReader};
use ffmpeg::{decoder, format, format::context, frame, media};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use std::thread::JoinHandle;
use std::time;
use tempfile::tempdir;

use error::ContextError;
use hook::Hook;
use input::{Input, InputPath};

pub type ContextResult<T = ()> = Result<T, ContextError>;

pub struct Context<'a, P: AsRef<Path>, S: AsRef<str>> {
    pub dest: PathBuf,
    pub input_path: InputPath,
    pub input: &'a Input,
    pub ictx: context::Input,
    hooks: Vec<&'a Hook>,
    tasks: Vec<Task<P, S>>,
}

pub enum Frame {
    Video(frame::Video),
    Audio(frame::Audio),
}

#[derive(Copy, Clone)]
pub struct Task<P: AsRef<Path>, S: AsRef<str>> {
    pub width: u32,
    pub height: u32,
    pub output_file: P,
    pub encoder: S,
    pub format: S,
}

impl<'a, P: AsRef<Path>, S: AsRef<str>> Context<'a, P, S> {
    pub fn new(input: &'a Input) -> ContextResult<Self> {
        let input_path = input.path()?;
        let dest_dir = tempdir().map_err(ContextError::Io)?;
        let ictx = match &input_path {
            InputPath::Path(p) => format::input(&p),
            InputPath::TempPath(p) => format::input(&p.to_path_buf()),
        }
        .map_err(ContextError::FFmpeg)?;

        Ok(Self {
            dest: dest_dir.path().to_path_buf(),
            input_path,
            hooks: vec![],
            tasks: vec![],
            ictx,
            input,
        })
    }

    pub fn add_hook(&mut self, hook: &'a Hook) -> &mut Self {
        self.hooks.push(hook);
        self
    }

    pub fn add_task(&mut self, task: Task<P, S>) -> &mut Self {
        self.tasks.push(task);
        self
    }

    pub fn start_encoder(
        &self,
        mut rx: BusReader<Arc<Frame>>,
        task: &Task<P, S>,
    ) -> ContextResult<JoinHandle<Result<(), ffmpeg::Error>>> {
        let mut octx = format::output_as(&task.output_file.as_ref(), &task.format.as_ref())
            .map_err(ContextError::FFmpeg)?;

        octx.set_metadata(self.ictx.metadata().to_owned());

        //octx.write_header().map_err(ContextError::FFmpeg)?;
        Ok(thread::spawn(move || {
            // Set timeout to 10 minutes.
            let timeout = time::Duration::from_secs(60 * 10);
            while let Ok(frame) = rx.recv_timeout(timeout) {
                match frame.as_ref() {
                    Frame::Video(video) => {
                        println!(
                            "Video frame: {}x{} {:?}",
                            video.width(),
                            video.height(),
                            video.timestamp()
                        );
                    }
                    Frame::Audio(audio) => println!("Audio frame: {:?}", audio.timestamp()),
                };
            }

            //octx.write_trailer()

            Ok(())
        }))
    }

    pub fn get_audio_stream_and_decoder(&mut self) -> ContextResult<(usize, decoder::Audio)> {
        let stream = self
            .ictx
            .streams()
            .best(media::Type::Audio)
            .ok_or_else(|| ContextError::Error("Unable to find audio stream".into()))?;

        Ok((
            stream.index(),
            stream
                .codec()
                .decoder()
                .audio()
                .map_err(ContextError::FFmpeg)?,
        ))
    }

    pub fn get_video_stream_and_decoder(&mut self) -> ContextResult<(usize, decoder::Video)> {
        let stream = self
            .ictx
            .streams()
            .best(media::Type::Video)
            .ok_or_else(|| ContextError::Error("Unable to find video stream".into()))?;

        Ok((
            stream.index(),
            stream
                .codec()
                .decoder()
                .video()
                .map_err(ContextError::FFmpeg)?,
        ))
    }

    pub fn run(&mut self) -> ContextResult {
        if self.tasks.is_empty() {
            return Err(ContextError::Error("No task".into()));
        }

        let mut video = self.get_video_stream_and_decoder();
        let mut audio = self.get_audio_stream_and_decoder();

        if video.is_err() && audio.is_err() {
            return Err(ContextError::Error(
                "Failed to find video and audio streams".into(),
            ));
        }

        let mut bus: Bus<Arc<Frame>> = Bus::new(self.tasks.len() - 1);
        let mut threads = Vec::new();

        for task in self.tasks.iter() {
            let rx = bus.add_rx();
            let thread = self.start_encoder(rx, task)?;
            threads.push(thread);
        }

        for (stream, packet) in self.ictx.packets() {
            if let Ok((index, decoder)) = &mut video {
                if *index == stream.index() {
                    let mut decoded = frame::Video::empty();
                    decoder
                        .decode(&packet, &mut decoded)
                        .map_err(ContextError::FFmpeg)?;
                    bus.broadcast(Arc::new(Frame::Video(decoded)));
                }
            }
            if let Ok((index, decoder)) = &mut audio {
                if *index == stream.index() {
                    let mut decoded = frame::Audio::empty();
                    decoder
                        .decode(&packet, &mut decoded)
                        .map_err(ContextError::FFmpeg)?;
                    bus.broadcast(Arc::new(Frame::Audio(decoded)));
                }
            }
        }

        std::mem::drop(bus);

        for thread in threads {
            thread
                .join()
                .map_err(|_| ContextError::Error("Failed to join thread".into()))?
                .map_err(ContextError::FFmpeg)?;
        }

        for hook in self.hooks.iter() {
            hook.execute()?;
        }

        self.hooks.clear();
        self.tasks.clear();

        Ok(())
    }
}
