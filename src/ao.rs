//! libao sink

use libao;
use super::{Source, Sink};

/// Sink writing to a libao device.
///
/// Consumes samples of format `F` from a `Source` `R`.
pub struct AOSink<'a, F, R> {
    device: libao::Device<'a, F>,
    source: R
}

impl<'a, F: libao::Sample, R: Source<F>> AOSink<'a, F, R> {
    /// Construct a libao sink.
    pub fn new<'a>(source: R, driver: &libao::Driver<'a>) -> libao::AoResult<AOSink<'a, F, R>> {

        // TODO permit user to specify these parameters
        let format = libao::SampleFormat {
            sample_rate: 44100,
            channels: 1,
            byte_order: libao::Native,
            matrix: None
        };

        Ok(AOSink {
            device: match driver.get_info().unwrap().flavor {
                libao::Live => {
                    try!(driver.open_live(&format))
                },
                libao::File => {
                    fail!("Can't do file output yet.")
                }
            },
            source: source,
        })
    }
}

impl<'a, F: libao::Sample, R: Source<F>> Sink for AOSink<'a, F, R> {
    fn run_once(&mut self) -> Option<()> {
        let samples = match self.source.next() {
            None => {
                return None;
            },
            Some(buffer) => buffer,
        };
        self.device.play(samples);
        Some(())
    }
}
