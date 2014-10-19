//! Ogg vorbis decoder.

// Future work: permit compile-time selection of the default vorbis plugin:
// Xiph.org libvorbisfile, or rust-vorbis.
// #[cfg(libvorbis = "xiph")]
// use xiph_vorbis;
// #[cfg(libvorbis = "rust")]
// use rust_vorbis;

extern crate "libvorbisfile" as vorbisfile;

use super::{Source, SourceResult, Buffer, StreamError, EndOfStream};
use self::vorbisfile::OVResult;

/// Ogg Vorbis decoder.
pub struct VorbisStream<R> {
    src: vorbisfile::VorbisFile<R>,
}

impl<R: Reader> VorbisStream<R> {
    /// Open a new decoder.
    pub fn open(reader: R) -> OVResult<VorbisStream<R>> {
        Ok(VorbisStream {
            src: try!(vorbisfile::VorbisFile::new(reader))
        })
    }
}

// The native result type for vorbis is a C float. ov_read() postprocesses into
// integer samples, which we're equally capable of doing.
impl<R: Reader> Source<f32> for VorbisStream<R> {
    fn next<'a>(&'a mut self) -> SourceResult<'a, f32> {
        // TODO report sample rate
        match self.src.decode() {
            Ok(b) => Buffer(b),
            // ??? => SampleRate(...),
            Err(vorbisfile::EndOfStream) => EndOfStream,
            Err(e) => StreamError(format!("vorbisfile decoder: {}", e))
        }
    }
}
