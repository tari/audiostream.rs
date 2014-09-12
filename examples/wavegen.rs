extern crate ao;
extern crate audiostream;

use audiostream::{Sink, MonoSource, Source};
use audiostream::synth::{Null, Tone};
use audiostream::ao::AOSink;
use std::io;
use std::os;
use std::sync::Arc;
use std::sync::atomics::{AtomicBool, Release};

fn usage() {
    let args = os::args();
    let name = match args.as_slice().get(0) {
        Some(s) => s.as_slice(),
        None => "wavegen"
    };
    println!("Usage: {} [silence | sin]", name);
}

#[allow(non_snake_case)]
fn main() {
    let terminate = Arc::new(AtomicBool::new(false));
    // Will move into the pipeline thread, and we don't need it here
    // beyond requiring that it be initialized in the main thread.
    let AO = ao::AO::init();

    let args = os::args();
    if args.len() != 2 {
        usage();
        return;
    }

    {
        let terminate = terminate.clone();

        spawn(proc() {
            // TODO would like DST so we don't need to box 'em.
            let generator: Box<Source<i16>> = match args[1].as_slice() {
                "silence" => box Null::<i16>::new(4096).adapt() as Box<Source<i16>>,
                "sin" => box Tone::<i16>::new(4096, 44100 / 440).adapt() as Box<Source<i16>>,
                _ => {
                    usage();
                    return;
                }
            };

            let driver = match AO.get_driver("") {
                None => {
                    println!("Failed to get default libao driver");
                    return;
                }
                Some(driver) => driver
            };
            let sink = AOSink::<i16, _>::new(
                generator,
                &driver
            );

            let mut sink = match sink {
                Err(e) => {
                    println!("Failed to open output device: {}", e);
                    return;
                }
                Ok(s) => s
            };
            println!("Press ENTER to exit.")
            sink.run(terminate.deref());
        })
    }

    match io::stdin().read_line() {
        Ok(_) => {
            terminate.store(true, Release);
            println!("Terminating.")
        }
        Err(e) => println!("I/O error on stdin: {}", e),
    }
}
