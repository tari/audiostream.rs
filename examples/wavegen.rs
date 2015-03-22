#![feature(plugin)]
#![plugin(docopt_macros)]

extern crate ao;
extern crate audiostream;
extern crate docopt;
extern crate "rustc-serialize" as rustc_serialize;

use audiostream::{Sink, MonoSource, Source, Amplify};
use audiostream::synth::{Null, Tone};
use audiostream::ao::AOSink;
use std::io::{self, BufRead};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

docopt!(Args, "
Usage: wavegen [options] [WAVEFORM]

Options:
    -a AMP, --amplitude=AMP  Adjust amplitude of output waveform [default: 1.0]
    -h, --help               Show this message.

The following waveforms are supported:
 * sin: 440 Hz sinusoid
 * silence: null signal
", flag_amplitude: Option<f32>);

#[allow(non_snake_case)]
fn main() {
    let args: Args = Args::docopt().decode().unwrap_or_else(|e| e.exit());

    let waveform = args.arg_WAVEFORM;
    let amplitude = args.flag_amplitude.unwrap_or(1.0);

    let terminate = Arc::new(AtomicBool::new(false));
    // Will move into the pipeline thread, and we don't need it here
    // beyond requiring that it be initialized in the main thread.
    let AO = ao::AO::init();

    {
        let terminate = terminate.clone();

        thread::spawn(move|| {
            let generator: Box<Source<Output=i16>> = match &waveform[..] {
                "silence" => Box::new(
                    Null::<i16>::new(4096).adapt()
                ) as Box<Source<Output=i16>>,
                x @ "sin" | x => {
                    if x != "sin" {
                        println!("Unrecognized waveform: `{}', defaulting to `sin'", x);
                    }
                    Box::new(
                        Tone::<i16>::new(4096, 44100 / 440).adapt()
                    ) as Box<Source<Output=i16>>
                }
            };

            let driver = match AO.get_driver("") {
                None => {
                    panic!("Failed to open libao default driver");
                }
                Some(driver) => driver
            };
            let sink = AOSink::<i16, _>::new(
                Amplify::<_, _, f32>::new(generator, amplitude),
                &driver
            );

            let mut sink = match sink {
                Err(e) => {
                    println!("Failed to open output device: {}", e);
                    return;
                }
                Ok(s) => s
            };
            println!("Press ENTER to exit.");
            sink.run(&*terminate);
        });
    }

    let mut s = String::new();
    let mut _stdin = io::stdin();
    let mut stdin = _stdin.lock();
    match stdin.read_line(&mut s) {
        Ok(_) => {
            terminate.store(true, Ordering::Release);
            println!("Terminating.")
        }
        Err(e) => println!("I/O error on stdin: {}", e),
    }
}
