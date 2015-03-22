//! libao sink

extern crate ao;

use std::mem;
use super::{SourceResult, Sample, Source, Sink};
use super::interleave::Interleave;

/// Sink writing to a libao device.
///
/// Consumes samples of format `F` from a `Source` `R`.
pub struct AOSink<'a, F, R> {
    device: ao::Device<'a, F>,
    interleave_buf: Vec<F>,
    source: R,
}

impl<'a, F, R> AOSink<'a, F, R>  where
        F: ao::Sample,
        R: Source<Output=F> {
    /// Construct a libao sink.
    pub fn new(source: R, driver: &ao::Driver<'a>) -> ao::AoResult<AOSink<'a, F, R>> {

        // TODO permit user to specify these parameters
        let format = ao::SampleFormat::<F, &str>::new(44100, 1, ao::Endianness::Native, None);

        Ok(AOSink {
            device: match driver.get_info().unwrap().flavor {
                ao::DriverType::Live => {
                    try!(driver.open_live(&format))
                },
                ao::DriverType::File => {
                    panic!("Can't do file output yet.")
                }
            },
            interleave_buf: Vec::new(),
            source: source,
        })
    }
}

impl<'a, F: ao::Sample + Interleave, R: Source<Output=F>> Sink for AOSink<'a, F, R> {
    fn run_once(&mut self) -> Option<()> {
        match self.source.next() {
            SourceResult::Buffer(channels) => {
                // Interleave channels
                let len = channels[0].len();
                self.interleave_buf.reserve(len);
                unsafe {
                    self.interleave_buf.set_len(len);
                    // Transmute hack to lose `mut` on each channel.
                    Interleave::interleave(mem::transmute(channels), self.interleave_buf.as_mut_slice());
                }

                self.device.play(self.interleave_buf.as_slice());
                // Drop all interleaved samples
                self.interleave_buf.truncate(0);
                Some(())
            }
            _ => None
        }
    }
}

/// Dynamic-format AO output.
#[warn(dead_code)]
pub struct AOAutoWriterSink<'a, R, W, _S> {
    /// Writer which receives data from libao
    dest: W,
    /// libao device handle
    device: ao::auto::AutoFormatDevice<'a, _S>,
    /// Source this receives data from.
    source: R
}

// TODO we really want dynamic format support for sinks here.
/*impl<'a, R, W, _S> AOAutoSink<'a, R, W, _S> where
        R: DynamicSource,
        W: Writer,
        _S: Str {

}*/
