#![crate_name = "audiostream"]
#![doc(html_root_url = "http://www.rust-ci.org/tari/audiostream.rs/doc/audiostream/")]

#![experimental]
#![deny(dead_code,missing_doc)]

#![feature(asm)]
#![feature(default_type_params)]
#![feature(macro_rules)]
#![feature(phase)]
#![feature(simd)]

//! Audio steam pipelines and processing.
//! 
//! Streams are represented as sequences of buffers, each of which contains
//! zero or more samples. Data is produced from `Source`s on demand and fed
//! into a chain of zero or more `Sink`s until it reaches the end of the
//! pipeline. The pipeline always operates in a "pull" mode, where `Source`s
//! yield buffers only as fast as requested by a `Sink`.
//! 
//! Valid sample formats are represented with the `Sample` trait. In general,
//! buffers are of type `&[Sample]`, presenting a single channel of audio
//! data. However, this convention is not enforced.

extern crate "ao" as libao;

#[phase(plugin)]
extern crate lazy_static;
#[cfg(test)]
extern crate test;

use std::mem;
use std::num::Zero;
use std::slice::mut_ref_slice;
use std::sync::atomics::{AtomicBool, Acquire};

mod cpu;
mod interleave;

// #{cfg(libao)]
pub mod ao;
pub mod synth;
// #[cfg(libvorbis)]
pub mod vorbis;

/// Type bound for sample formats.
/// 
/// Implementation assumes `f64` is sufficient to represent all other formats
/// without loss.
// Interleave bound is a little wonky, but necessary because we can't have closed typeclasses nor
// both generic (T: Sample) and specialized (i16) impls for a given trait.
pub trait Sample : Num + NumCast + interleave::Interleave {
    /// Maximum value of a valid sample.
    fn max() -> Self;
    /// Minimum value of a valid sample.
    fn min() -> Self;
    /// Clip a value to be in range [min, max] (inclusive).
    fn clip(&self) -> Self;

    /// Convert from `Self` to an arbitrary other sample format.
    /// 
    /// The default intermediate format here is `f64`, capable of losslessly
    /// converting all formats shorter than 52 bits.
    fn convert<X: Sample, I: Sample = f64>(a: Self) -> X {
        let a_i: I = NumCast::from(a).unwrap();
        let self_max: Self = Sample::max();
        let self_max_i: I = NumCast::from(self_max).unwrap();
        let ratio: I = a_i / self_max_i;

        let x_max: X = Sample::max();
        let x_max_i: I = NumCast::from(x_max).unwrap();

        NumCast::from(ratio * x_max_i).unwrap()
    }
}

macro_rules! sample_impl(
    ($t:ty, $min:expr .. $max:expr) => (
        impl Sample for $t {
            fn max() -> $t { $min }
            fn min() -> $t { $max }
            fn clip(&self) -> $t {
                if *self < Sample::min() {
                    Sample::min()
                } else if *self > Sample::max() {
                    Sample::max()
                } else {
                    *self
                }
            }
        }
    );
    ($t:ty) => (
        sample_impl!($t, ::std::num::Bounded::min_value()
                      .. ::std::num::Bounded::max_value())
    )
)
sample_impl!(i8)
sample_impl!(i16)
// Conspicuously missing: i24
sample_impl!(i32)
sample_impl!(f32, -1.0 .. 1.0)
sample_impl!(f64, -1.0 .. 1.0)

/// Output from `Source` pull.
pub enum SourceResult<'a, T:'a> {
    /// Channel-major buffer of samples.
    ///
    /// All channels are guaranteed to have the same number of samples, and there is always at
    /// least one channel.
    Buffer(&'a mut [&'a mut [T]]),
    /// Following samples have the specified rate (in Hz).
    SampleRate(uint),
    /// Reached stream end.
    EndOfStream,
    /// There was an error in the stream.
    StreamError(String),
}

/// A source of samples.
/// 
/// Generates buffers of samples of type `T` and passes them to a consumer.
pub trait Source<T> {
    /// Emit the next buffer.
    fn next<'a>(&'a mut self) -> SourceResult<'a, T>;
}

impl<F> Source<F> for Box<Source<F>+'static> {
    fn next<'a>(&'a mut self) -> SourceResult<'a, F> {
        self.next()
    }
}

/// A `Source` that only generates one channel at an indeterminate sample rate.
/// 
/// To generalize to a full `Source`, use the `adapt` method.
pub trait MonoSource<T> {
    /// Get the next set of samples.
    fn next<'a>(&'a mut self) -> Option<&'a mut [T]>;

    /// Adapts a `MonoSource` into a (more general) `Source`.
    fn adapt(self) -> MonoAdapter<T, Self> {
        MonoAdapter {
            src: self,
            bp: ::std::raw::Slice {
                data: ::std::ptr::null(),
                len: 0
            }
        }
    }
}

/// Generalizes a `MonoSource` into `Source`.
/// 
/// To get one, use `MonoSource::adapt`.
pub struct MonoAdapter<F, T> {
    src: T,
    bp: ::std::raw::Slice<F>
}

impl<F, T: MonoSource<F>> Source<F> for MonoAdapter<F, T> {
    fn next<'a>(&'a mut self) -> SourceResult<'a, F> {
        // bp is a bit of a hack, since a function-local can't live long enough to be returned. We
        // drop the slice into a struct-private field so the pointers remain live, and it remains
        // safe because the pointer chain is as follows:
        //     caller -> self.bp -> self.src
        // 'a bounds self, so the lifetime is valid for both bp and src.
        self.bp = match self.src.next() {
            None => return EndOfStream,
            Some(b) => unsafe { mem::transmute(b) }
        };
        
        Buffer(mut_ref_slice(unsafe {
            mem::transmute(&mut self.bp)
        }))
    }
}

/// A thing.
pub trait Sink {
    /// Process a single buffer.
    ///
    /// Returns `Some` if there will be more buffers to process, or `None`
    /// otherwise.
    fn run_once(&mut self) -> Option<()>;

    /// Process buffers indefinitely, until end of stream or terminated.
    ///
    /// As long as `term_cond` is `true` and there are buffers available,
    /// this will process buffers. If `term_cond` is cleared, no additional
    /// buffers will be processed and the function returns.
    ///
    /// If `term_cond` is never modified, this is equivalent to repeatedly
    /// calling `run_once` until it returns `None`.
    fn run(&mut self, term_cond: &AtomicBool) {
        loop {
            if term_cond.load(Acquire) || self.run_once().is_none() {
                return;
            }
        }
    }
}

/// A source of uncontrolled samples.
/// 
/// Owns buffers that get passed down through a pipeline, providing no
/// guarantees about what's in the buffer beyond that it's safe to read
/// and write.
/// 
/// This struct is used internally by most synthesis sources, and is
/// generally not useful to library users. It may be useful, however,
/// for building custom sources.
pub struct UninitializedSource<F> {
    buffer: Vec<F>
}

impl<F: Sample> UninitializedSource<F> {
    /// Create a source of uncontrolled samples.
    /// 
    /// The yielded buffers will have `size` items.
    pub fn new(size: uint) -> UninitializedSource<F> {
        UninitializedSource {
            buffer: Vec::from_fn(size, |_| Zero::zero())
        }
    }
}

impl<F> MonoSource<F> for UninitializedSource<F> {
    fn next<'a>(&'a mut self) -> Option<&'a mut [F]> {
        Some(self.buffer.as_mut_slice())
    }
}
