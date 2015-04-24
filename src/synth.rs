//! Signal synthesizers.

use num::{NumCast, Float, FromPrimitive};
use super::{Sample, MonoSource, UninitializedSource};
use std::f64::consts::PI_2;
use std::iter::Cycle;
use std::marker::PhantomData;
use std::ops::Range;

use rand::Rng;
use rand::distributions::{IndependentSample, Normal};

#[cfg(test)]
use test::Bencher;

/// Pure silence.
pub struct Null<F> {
    src: UninitializedSource<F>
}

impl<F: Sample> Null<F> {
    /// Create a source of pure silence for buffers of `size` samples.
    pub fn new(size: usize) -> Null<F> {
        Null {
            src: UninitializedSource::new(size)
        }
    }
}

impl<F: Sample> MonoSource for Null<F> {
    type Output = F;

    fn next<'a>(&'a mut self) -> Option<&'a mut [F]> {
        self.src.next().map(|buf| {
            for x in buf.iter_mut() {
                *x = FromPrimitive::from_usize(0).unwrap();
            }
            buf
        })
    }
}

#[bench]
fn generate_silence(b: &mut Bencher) {
    let bufsize = 4096;
    let mut src = Null::<i16>::new(bufsize);
    b.bytes = ::std::mem::size_of::<i16>() as u64 * bufsize as u64;
    b.iter(|| {
        src.next();
    });
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
pub struct Tone<F, P=f32> {
    src: UninitializedSource<F>,
    timebase: Cycle<Range<usize>>,
    period: usize,
    gentype: PhantomData<P>
}

impl<F: Sample, P = f32> Tone<F, P> {
    /// Create a pure tone generator with a specified period in samples for
    /// buffers of `size` samples.
    pub fn new(size: usize, period: usize) -> Tone<F, P> {
        Tone {
            src: UninitializedSource::new(size),
            timebase: (0..period).cycle(),
            period: period,
            gentype: PhantomData
        }
    }
}

// TODO Float is kinda slow-feeling. Prefer a custom Sinusoid
// trait that can avoid floats sometimes.
impl<F: Sample, P: Sample+Float> MonoSource for Tone<F, P> {
    type Output = F;

    fn next<'a>(&'a mut self) -> Option<&'a mut [F]> {
        let buf = match self.src.next() {
            Some(b) => b,
            None => return None
        };

        for (x, t) in buf.iter_mut().zip(self.timebase.by_ref()) {
            let mut y: P = NumCast::from(t).unwrap();
            y = y * NumCast::from(PI_2).unwrap();
            y = y / NumCast::from(self.period).unwrap();
            *x = Sample::convert::<F>(y.sin());
        }
        Some(buf)
    }
}

#[bench]
fn generate_a440_44100(b: &mut Bencher) {
    let bufsize = 4096;
    let mut src = Tone::<i16>::new(bufsize, 100);
    b.bytes = ::std::mem::size_of::<i16>() as u64 * bufsize as u64;
    b.iter(|| {
        src.next();
    });
}

/// Pure Gaussian white noise.
pub struct WhiteNoise<F, R> {
    rng: R,
    normal: Normal,
    src: UninitializedSource<F>
}

impl<R: Rng> WhiteNoise<f64, R> {
    /// Create a white noise generator for buffers of `size` samples.
    pub fn new(size: usize, rng: R) -> WhiteNoise<f64, R> {
        WhiteNoise {
            rng: rng,
            normal: Normal::new(0f64, 0.25),
            src: UninitializedSource::new(size)
        }
    }
}

impl<R: Rng> MonoSource for WhiteNoise<f64, R> {
    type Output = f64;

    fn next<'a>(&'a mut self) -> Option<&'a mut [f64]> {
        let buf = match self.src.next() {
            Some(b) => b,
            None => return None
        };

        for x in buf.iter_mut() {
            *x = self.normal.ind_sample(&mut self.rng).clip();
        }
        Some(buf)
    }
}

#[bench]
fn generate_xorshift_noise_44100(b: &mut Bencher) {
    use super::MonoSource;
    use rand;

    let bufsize = 4096;
    let mut src = WhiteNoise::new(bufsize, rand::XorShiftRng::new_unseeded());
    b.bytes = ::std::mem::size_of::<f64>() as u64 * bufsize as u64;

    b.iter(|| {
        src.next();
    });
}
