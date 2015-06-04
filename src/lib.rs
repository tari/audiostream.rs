#![crate_name = "audiostream"]
#![doc(html_root_url = "http://rustdoc.taricorp.net/audiostream/audiostream/")]

#![deny(dead_code,missing_docs)]

#![feature(asm)]
#![feature(core)]
#![feature(custom_attribute)]
#![feature(plugin)]
#![feature(simd)]
#![feature(slice_patterns)]
#![feature(str_char)]
#![plugin(quickcheck_macros)]

//! Audio stream pipelines and processing.
//! 
//! Streams are represented as sequences of buffers, each of which contains zero or more samples.
//! Data is produced from `Source`s on demand and fed into a chain of zero or more `Sink`s until it
//! reaches the end of the pipeline. The pipeline always operates in a "pull" mode, where `Source`s
//! yield buffers only as fast as requested by a `Sink`.
//! 
//! ## Samples
//! 
//! Valid sample formats are bounded by the `Sample` trait. In general a sample will be a primitive
//! numeric value, though this need not be true. Sample values must always be copyable and sendable
//! between threads, and most non-trivial stream transformations require that a number of
//! arithmatic operations be available.
//!
//! ### Clipping
//!
//! In all formats, the nominal range is between -1 and 1, inclusive. In integer formats, the
//! logical interpretation is as a fixed-point value with the radix point left-aligned. For
//! example, a `i8` `Sample` is best considered as a number in range -128/128 through 127/128 by
//! steps of 1/128.
//!
//! A format is considered *soft-clipped* if it is capable of representing values outside the
//! nominal range. Notably, this applies to floating-point formats where numbers outside the
//! nominal range can be represented (but perhaps with some loss of precision). The converse of a
//! hard-clipped format is *soft-clipped*.
//!
//! ## Source taxonomy
//!
//! From least general to most, there are three classes of sources, each of which yield different
//! "flavors" of output.
//!  * `MonoSource`s simply provide a stream of samples of a statically-known format. This stream
//!  is strictly linear and can only mark end-of-stream.
//!  * `Source` is the general static element, providing blocks of a statically-known sample
//!  format. It may pass an arbitrary number of channels at a time and specifies the stream's
//!  sample rate.
//!  * A `DynamicSource` has no properties known at compile-time. Sample format and rate are
//!  specified on a per-buffer basis, requiring reinterpretation of the data before use.
//!  
//! A less-general source can always be adapted into a more-general source. A `MonoAdapter`
//! converts `MonoSource` to `Source`, and `DynAdapter` converts `Source` to `DynamicSource`.

#[macro_use] extern crate lazy_static;
#[macro_use] extern crate log;
#[cfg(test)] extern crate test;
#[cfg(test)] extern crate quickcheck;

extern crate fftw3;
extern crate num;
extern crate rand;

use num::{NumCast, Float, FromPrimitive};
use std::marker::PhantomData;
use std::mem;
use std::ops::{Add, Mul, Div};
use std::raw;
use std::raw::Repr;
use std::slice::mut_ref_slice;
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(feature = "ao")] pub mod ao;
pub mod fft;
pub mod synth;
#[cfg(feature = "vorbisfile")] pub mod vorbis;

mod interleave;
#[cfg(target_arch = "x86_64")] mod cpu;

/// Type bound for sample formats.
pub trait Sample : Add<Self> + Mul<Self> + Div<Self> + OverflowingOps
                 + NumCast + FromPrimitive + ::std::fmt::Debug
                 + Copy + Send {

    /// Maximum value of a valid sample.
    fn max() -> Self;
    /// Minimum value of a valid sample.
    fn min() -> Self;
    /// True if this type has a hard limit on values in range [min, max].
    ///
    /// If false, values outside this range are representable and may be used but may incur loss of
    /// precision.
    fn clips_hard() -> bool;
    /// Clip a value to be in range [min, max] (inclusive).
    fn clip(&self) -> Self;

    /// Add two samples together, clipping if necessary (in hard-clipped formats).
    fn mix(&self, other: &Self) -> Self {
        if !self.clips_hard() {
            return self + other;
        }

        let (overflowed, result) = self.overflowing_add(other);
        if !overflowed {
            result
        } else {
            // Overflow can only occur if both values have the same sign, so
            // examining the sign of `self` only is correct.
            if self.is_positive() {
                self.max()
            } else {
                self.min()
            }
        }
    }

    /// Get a floating-point representation of a sample.
    ///
    /// Full-scale output is in the range -1 to 1. Soft-clipped types may
    /// yield values outside this range.
    fn to_float<F: Float + Sample>(x: Self) -> F {
        let f: F = NumCast::from(x).unwrap();
        let self_max: Self = Sample::max();
        let f_max: F = NumCast::from(self_max).unwrap();
        return f / f_max;
    }

    /// Convert a floating-point sample to any other format.
    ///
    /// Values outside the normal sample range in soft-clipped formats will
    /// not be clipped. When converting to a hard-clipped format, clipping
    /// may occur.
    fn from_float<F: Float + Sample>(mut x: F) -> Self {
        if <Self as Sample>::clips_hard() {
            x = x.clip();
        }

        let self_max: Self = Sample::max();
        let self_max_f: F = NumCast::from(self_max).unwrap();

        let out: Self = NumCast::from(self_max_f * x).unwrap();
        out
    }

    /// Convert from `Self` to an arbitrary other sample format.
    ///
    /// The default intermediate format here is `f64`, capable of losslessly
    /// converting all formats shorter than 52 bits. For shorter input formats
    /// (such as i16), f32 is sufficient for lossless conversion.
    fn convert<X: Sample, I: Float + Sample = f64>(a: Self) -> X {
        <X as Sample>::from_float(Sample::to_float::<I>(a))
    }
}

macro_rules! sample_impl(
    ($t:ty, $range:expr, $hard:expr) => (
        impl Sample for $t {
            #[inline]
            fn max() -> $t { $range.end }
            #[inline]
            fn min() -> $t { $range.start }
            #[inline]
            fn clips_hard() -> bool { $hard }
            #[inline]
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
    // Implicitly soft-clipped by specified range
    ($t:ty, $range:expr) => (
        sample_impl!($t, $range, false);
    );
    // Implicitly hard-clipped by type's range
    ($t:ident) => (
        sample_impl!($t, $t::min_value()
                      .. $t::max_value(), true);
    );
);
sample_impl!(i8);
sample_impl!(i16);
// Conspicuously missing: i24. Probably not a big deal, if we follow ffmpeg's
// precedent and sign-extend i24 for input.
sample_impl!(i32);
sample_impl!(f32, -1.0 .. 1.0);
sample_impl!(f64, -1.0 .. 1.0);

#[test]
fn test_impl_ranges() {
    // Implicit ranges
    assert_eq!(<i16 as Sample>::max(), 32767);
    assert_eq!(<i16 as Sample>::min(), -32768);
    assert_eq!(<i16 as Sample>::clips_hard(), true);
    assert_eq!(0i16.clip(), 0);
    assert_eq!(32767.clip(), 32767);
    
    // Explicit ranges
    assert_eq!(<f32 as Sample>::max(), 1f32);
    assert_eq!(<f32 as Sample>::min(), -1f32);
    assert_eq!(<f32 as Sample>::clips_hard(), false);
    assert_eq!(0f32.clip(), 0f32);
    assert_eq!(-2f32.clip(), -1f32);
}

#[quickcheck]
fn float_roundtrip_is_lossless(x: i16) -> bool {
    x == Sample::from_float(Sample::to_float::<f32>(x))
}

/// Output from `Source` pull.
#[derive(Debug, PartialEq)]
pub enum SourceResult<'a, T:'a> {
    /// Channel-major buffer of samples.
    ///
    /// All channels are guaranteed to have the same number of samples, and there is always at
    /// least one channel.
    Buffer(&'a mut [&'a mut [T]]),
    /// Following samples have the specified rate (in Hz).
    SampleRate(u32),
    /// Reached stream end.
    EndOfStream,
    /// There was an error in the stream.
    StreamError(String),
}

/// A source of samples with defined sample rate.
///
/// Generates buffers of samples of type `T` and passes them to a consumer.
pub trait Source {
    /// The sample format emitted by this source.
    type Output: Sample;
    /// Emit the next buffer.
    fn next<'a>(&'a mut self) -> SourceResult<'a, Self::Output>;
}

impl<'z, F: Sample> Source for Box<Source<Output=F> + 'z> {
    type Output = F;

    fn next<'a>(&'a mut self) -> SourceResult<'a, F> {
        (**self).next()
    }
}

/// The result of pulling from a `DynamicSource`.
///
/// You probably shouldn't use this because it's experimental.
// XXX
pub struct DynBuffer<'z> {
    /// Raw bytes of sample data.
    /// TODO Any might be more appropriate, particularly for externally-defined sample formats.
    /// It's very easy for us to get confused by one of those.
    pub bytes: &'z mut [&'z mut [u8]],
    /// Size of individual samples, in bits.
    ///
    /// Note that it's impossible to tell what actual format
    pub sample_size: u8,
    /// Sample rate in Hz
    pub sample_rate: u32
}

/// A `Source` with format known only at runtime.
///
/// You probably shouldn't use this because it's experimental.
// XXX
pub trait DynamicSource {
    /// Pull the next buffer from the source
    fn next_dyn<'a>(&'a mut self) -> Option<DynBuffer<'a>>;
}

/// Adapts a normal `Source` into a `DynamicSource`.
#[warn(dead_code)]
pub struct DynAdapter<S> {
    sample_rate: u32,
    source: S
}

impl<S: Source> DynAdapter<S> {
    /// Construct a dynamic source adapter from a plain `Source`.
    pub fn from_source(source: S) -> DynAdapter<S> {
        DynAdapter {
            sample_rate: 0,
            source: source
        }
    }
}

/*impl<S> DynamicSource for DynAdapter<S> where S: Source {
    fn next_dyn<'a>(&'a mut self) -> Option<DynBuffer> {
        loop {
            match self.source.next() {
                SourceResult::EndOfStream |
                SourceResult::StreamError(_) => return None,
                SourceResult::SampleRate(sr) => self.sample_rate = sr,
                SourceResult::Buffer(b) => unsafe {
                    // Get bytes only. This transmute makes the len field
                    // of the inner slices wrong becasuse we're changing the
                    // contained type.
                    let mut b = mem::transmute::<&'a mut [&'a mut [<S as Source>::Output]],
                                                 &'a mut [raw::Slice<u8>]>(b);
                    // Correct the len field of channel buffers
                    for i in 0 .. b.len() {
                        b[i].len *= mem::size_of::<<S as Source>::Output>();
                    }
                    
                    return Some(DynBuffer {
                        bytes: mem::transmute::<&'a mut [raw::Slice<u8>],
                                                &'a mut [&'a mut [u8]]>(b),
                        sample_size: mem::size_of::<<S as Source>::Output>() as u8,
                        sample_rate: self.sample_rate
                    })
                }
            }
        }
    }
}*/

/// A `Source` that only generates one channel at an indeterminate sample rate.
///
/// To generalize to a full `Source`, use the `adapt` method.
pub trait MonoSource : Sized {
    /// The sample format yielded by this source.
    type Output;

    /// Get the next set of samples.
    fn next<'a>(&'a mut self) -> Option<&'a mut [Self::Output]>;

    /// Adapts a `MonoSource` into a (more general) `Source`.
    fn adapt(self) -> MonoAdapter<Self::Output, Self> {
        MonoAdapter {
            src: self,
            bp: raw::Slice {
                data: ::std::ptr::null(),
                len: 0
            }
        }
    }
}

/// Generalizes a `MonoSource` into `Source`.
pub struct MonoAdapter<F, T> {
    src: T,
    bp: raw::Slice<F>
}

impl<F, T> Source for MonoAdapter<F, T> where
        F: Sample,
        T: MonoSource<Output=F> {
    type Output = F;

    fn next<'a>(&'a mut self) -> SourceResult<'a, F> {
        // bp is a bit of a hack, since a function-local can't live long enough to be returned. We
        // drop the slice into a struct-private field so the pointers remain live, and it remains
        // safe because the pointer chain is as follows:
        //     caller -> self.bp -> self.src
        // 'a bounds self, so the lifetime is valid for both bp and src.
        self.bp = match self.src.next() {
            None => return SourceResult::EndOfStream,
            Some(b) => b.repr()
        };
        
        SourceResult::Buffer(unsafe {
            mem::transmute::<&mut [raw::Slice<F>], &'a mut [&'a mut [F]]>(
                mut_ref_slice(&mut self.bp)
            )
        })
    }
}

impl<F, T> ::std::ops::Deref for MonoAdapter<F, T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.src
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
            if term_cond.load(Ordering::Acquire) || self.run_once().is_none() {
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
    pub fn new(size: usize) -> UninitializedSource<F> {
        UninitializedSource {
            buffer: (0..size).map(|_| FromPrimitive::from_usize(0).unwrap()).collect()
        }
    }
}

impl<F> MonoSource for UninitializedSource<F> {
    type Output = F;

    fn next<'a>(&'a mut self) -> Option<&'a mut [F]> {
        Some(&mut self.buffer)
    }
}

/// Make a copy of a specified channel.
///
/// The source channel may be any index, and the destination may be an existing
/// channel (in which case the original data is lost) or one more than the highest
/// valid channel (in which case a new channel is created).
///
/// Due to mutability requirements for channel data, this always makes a copy.
pub struct CopyChannel<F, S> {
    /// Channel index (from 0) to copy from.
    from: usize,
    /// Channel index to copy to.
    to: usize,
    source: S,
    // Contents of `slices` must never outlive the scope in which they are
    // assigned to maintain safety. Covariant lifetime is used to allow the
    // concrete lifetime in `next<'a>()` to be stored within the struct.
    slices: Vec<raw::Slice<F>>,
    samples: Vec<F>,
}

impl<F: Sample, S> CopyChannel<F, S> where S: Source<Output=F> {
    /// Create a new `CopyChannel`.
    pub fn new(from: usize, to: usize, source: S) -> CopyChannel<F, S> {
        CopyChannel {
            from: from,
            to: to,
            source: source,
            slices: Vec::new(),
            samples: Vec::new()
        }
    }
}

impl<F: Sample, S: Source<Output=F>> Source for CopyChannel<F, S> {
    type Output = F;

    fn next<'a>(&'a mut self) -> SourceResult<'a, F> {
        let b: &'a mut [&'a mut [F]] = match self.source.next() {
            SourceResult::Buffer(b) => b,
            x => return x
        };

        assert!(self.from < b.len(), "CopyChannel source must be a valid channel index");
        assert!(self.to <= b.len(), "CopyChannel cannot copy from {} to {} with only {} channels",
                                    self.from, self.to, b.len());

        self.slices.clear();
        self.slices.extend(b.iter().map(|x: &&mut [F]| (*x).repr()));

        self.samples.clear();
        self.samples.extend(b[self.from].iter().map(|x| *x));
        if self.to == b.len() {
            self.slices.push(unsafe {
                mem::transmute::<&'a mut [F], raw::Slice<F>>(&mut self.samples)
            });
        } else {
            self.slices[self.to] = self.slices[self.from];
        }
        SourceResult::Buffer(unsafe {
            mem::transmute::<&mut [raw::Slice<F>],&'a mut [&'a mut [F]]>(&mut self.slices)
        })
    }
}

/// Adjust the amplitude of the input stream by a constant factor.
///
/// A factor greater than one increases amplitude, less than one reduced
/// amplitude.
#[allow(dead_code)]
pub struct Amplify<F, S, P> {
    factor: P,
    source: S,
    format: PhantomData<F>
}

impl<F, S, P> Amplify<F, S, P> {
    /// Create a new amplifier (or attenuator).
    pub fn new(source: S, factor: P) -> Amplify<F, S, P> {
        Amplify {
            factor: factor,
            source: source,
            format: PhantomData
        }
    }
}

impl<F: Sample, S: Source<Output=F>, P: Float + Sample> Source for Amplify<F, S, P> {
    type Output = F;

    fn next<'a>(&'a mut self) -> SourceResult<'a, F> {
        let buf = match self.source.next() {
            SourceResult::Buffer(b) => b,
            x => return x
        };

        // TODO must handle clipping somehow
        for channel in buf.iter_mut() {
            for sample in channel.iter_mut() {
                let samp_f: P = Sample::to_float::<P>(*sample);
                *sample = Sample::from_float(samp_f * self.factor);
            }
        }
        SourceResult::Buffer(buf)
    }
}

pub struct Mix<A, B> {
    sources: (A, B),
}

impl<A, B, F> Mix<A, B> where
        A: MonoSource<Output=F>, B: MonoSource<Output=F> {
    pub fn new(a: A, b: B) -> Mix<A, B> {
        Mix {
            sources: (a, b)
        }
    }
}

impl<A, B, F> MonoSource for Mix<A, B> where
        A: MonoSource<Output=F>, B: MonoSource<Output=F>, F: Sample {
    type Output = F;

    fn next<'a>(&'a mut self) -> Option<&'a mut [F]> {
        let a_buf = match self.sources.0.next() {
            Some(b) => b,
            x => return x
        };
        let b_buf = match self.sources.1.next() {
            Some(b) => b,
            x => return x
        };

        // TODO buffers must be the same length. Should either document that requirement
        // or make it handle irregular buffers.
        for i in 0..a_buf.len() {
            a_buf[i] = a_buf[i].mix(b_buf[i]);
        }

        a_buf
}

#[cfg(test)]
mod tests {
    use super::{Sample, Source, SourceResult, MonoSource, Amplify};

    struct ConstantSource<F> {
        data: Vec<F>,
        sbuf: Vec<F>
    }

    impl<F: Sample + Clone> MonoSource for ConstantSource<F> {
        type Output = F;

        fn next<'a>(&'a mut self) -> Option<&'a mut [F]> {
            self.sbuf = self.data.clone();
            Some(&mut self.sbuf)
        }
    }

    impl<F> ::std::default::Default for ConstantSource<F> {
        fn default() -> ConstantSource<F> {
            ConstantSource {
                data: vec![],
                sbuf: vec![]
            }
        }
    }


    #[quickcheck]
    fn copychannel_copies_channels(xs: Vec<i16>) -> bool {
        let mut src = super::CopyChannel::new(0, 1, ConstantSource::<i16> {
            data: xs.clone(),
            sbuf: vec![]
        }.adapt());
        if let SourceResult::Buffer(out) = src.next() {
            out[1] == &xs[..] && out[0] == out[1]
        } else {
            unreachable!();
        }
    }

    #[test]
    fn test_amplify() {
        let mut src = Amplify::<_, _, f32>::new(ConstantSource::<i16> {
                data: vec![0, 64, 128, 64, 0, -64, -128, -64, 0],
                sbuf: vec![]
            }.adapt(),
            1.0
        );

        assert_eq!(src.next(),
                   SourceResult::Buffer(
                       &mut [&mut [0i16, 64, 128, 64, 0, -64, -128, -64, 0]]
                   ));
    }
}
