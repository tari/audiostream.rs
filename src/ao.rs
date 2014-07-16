//! libao sink

use libao;
use super::{Source, Sink};

/// Type bound for sample formats supported by libao
trait AOSample : Int { }
impl AOSample for i8 { }
impl AOSample for i16 { }
impl AOSample for i32 { }


/// Sink writing to a libao device.
///
/// Consumes samples of format `F` from a `Source` `R`.
pub struct AOSink<'a, F, R> {
    device: libao::Device<'a, F>,
    source: R
}

impl<'a, F: AOSample, R: Source<F>> AOSink<'a, F, R> {
    /// Construct a libao sink.
    pub fn new<'a>(source: R, lib: &'a libao::AO, driver: &str,
                   options: &[(&str, &str)]) -> libao::AoResult<AOSink<'a, F, R>> {

        let driver = match lib.get_driver(driver) {
            Some(d) => d,
            None => {
                return Err(libao::NoDriver);
            }
        };

        // TODO permit user to specify these parameters
        let format = libao::SampleFormat {
            sample_rate: 44100,
            channels: 1,
            byte_order: libao::Native,
            matrix: None
        };

        Ok(AOSink {
            device: match driver.get_info(lib).unwrap().flavor {
                libao::Live => {
                    try!(libao::Device::live(lib, driver, &format))
                },
                libao::File => {
                    fail!("Can't do file output yet.")
                }
            },
            source: source,
        })
    }
}

impl<'a, F: AOSample, R: Source<F>> Sink for AOSink<'a, F, R> {
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
