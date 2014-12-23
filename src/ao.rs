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

impl<'a, F: ao::Sample, R: Source<F>> AOSink<'a, F, R> {
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

impl<'a, F: ao::Sample + Sample, R: Source<F>> Sink for AOSink<'a, F, R> {
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
