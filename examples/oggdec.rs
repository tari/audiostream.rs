extern crate ao;
extern crate audiostream;

use audiostream::Sink;
//use audiostream::ao::AOAutoSink;
use audiostream::vorbis::VorbisStream;
use std::io;

#[allow(non_snake_case)]
fn main() {
    let AO = ao::AO::init();
    let driver = match AO.get_driver("wav") {
        Some(d) => d,
        None => {
            println!("Failed to open AO 'wav' driver");
            return;
        }
    };

    // stdin -> VorbisStream -> AOAutoSink -> stdout
    /*let pipeline = AOAutoSink::for_writer(
        io::stdout(),
        &driver,
        VorbisStream::new(io::stdin())
    );*/

    //pipeline.run_to_end();
}
