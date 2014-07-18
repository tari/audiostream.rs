//! Signal synthesizers.

use super::{Sample, Source, UninitializedSource};
use std::f64::consts::PI_2;
use std::iter::{Range, Cycle};
use std::num::Zero;
use std::rand::Rng;
use std::rand::distributions::{IndependentSample, Normal};

/// Pure silence.
pub struct Null<F> {
    src: UninitializedSource<F>
}

impl<F: Sample> Null<F> {
    /// Create a source of pure silence for buffers of `size` samples.
    pub fn new(size: uint) -> Null<F> {
        Null {
            src: UninitializedSource::new(size)
        }
    }
}

impl<F: Sample> Source<F> for Null<F> {
    fn next<'a>(&'a mut self) -> Option<&'a mut [F]> {
        self.src.next().map(|buf| {
            for x in buf.mut_iter() {
                *x = Zero::zero();
            }
            buf
        })
    }
}

/// A pure tone.
/// 
/// The emitted signal is a full-scale (spans the entire range of the output
/// type) sin wave, starting at zero.
///
/// The optional type parameter `P` specifies the type in which the sin wave
/// will be generated. Some users may wish to use `f64` for greater precision
/// in signals with long period, or other types according to the application's
/// required precision.
pub struct Tone<F, P> {
    src: UninitializedSource<F>,
    timebase: Cycle<Range<uint>>,
    period: uint
}

impl<F: Sample, P> Tone<F, P> {
    /// Create a pure tone generator with a specified period in samples for
    /// buffers of `size` samples.
    pub fn new(size: uint, period: uint) -> Tone<F, P> {
        Tone {
            src: UninitializedSource::new(size),
            timebase: range(0, period).cycle(),
            period: period
        }
    }
}

// TODO FloatMath is kinda slow-feeling. Prefer a custom Sinusoid
// trait that can avoid floats.
impl<F: Sample, P: Sample+FloatMath = f32> Source<F> for Tone<F, P> {
    fn next<'a>(&'a mut self) -> Option<&'a mut [F]> {
        match self.src.next() {
            Some(buf) => {
                for (x, t) in buf.mut_iter().zip(self.timebase) {
                    let mut y: P = NumCast::from(t).unwrap();
                    y = y * NumCast::from(PI_2).unwrap();
                    y = y / NumCast::from(self.period).unwrap();
                    *x = Sample::convert::<F>(y.sin());
                }
                Some(buf)
            }
            None => unreachable!()
        }
    }
}

/// Pure Gaussian white noise.
pub struct WhiteNoise<F, R> {
    rng: R,
    normal: Normal,
    src: UninitializedSource<F>
}

impl<R: Rng> WhiteNoise<f64, R> {
    /// Create a white noise generator for buffers of `size` samples.
    pub fn new(size: uint, rng: R) -> WhiteNoise<f64, R> {
        WhiteNoise {
            rng: rng,
            normal: Normal::new(0f64, 0.25),
            src: UninitializedSource::new(size)
        }
    }
}

impl<R: Rng> Source<f64> for WhiteNoise<f64, R> {
    fn next<'a>(&'a mut self) -> Option<&'a mut [f64]> {
        match self.src.next() {
            Some(buf) => {
                for x in buf.mut_iter() {
                    *x = self.normal.ind_sample(&mut self.rng).clip();
                }
                Some(buf)
            }
            None => unreachable!()
        }
    }
}
