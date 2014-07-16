#![experimental]
#![deny(dead_code,missing_doc)]

//! Audio steam pipelines and processing.

extern crate libao = "ao";

use std::num::Zero;
use std::sync::atomics::{AtomicBool, AcqRel};

pub mod ao;

/// Type bound for sample formats.
pub trait Sample : Num { }
impl Sample for i8 { }
impl Sample for i16 { }
// Conspicuously missing: i24
impl Sample for i32 { }
impl Sample for f32 { }
impl Sample for f64 { }

/// A source of samples.
/// 
/// Generates buffers of samples of type `T` and passes them to a consumer.
pub trait Source<T> {
    /// Emit the next buffer.
    fn next<'a>(&'a mut self) -> Option<&'a mut [T]>;
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
    /// If `term_cond` is never modified, this is equivalent to calling
    /// `run_once` until it returns `None`.
    fn run(&mut self, term_cond: &AtomicBool) {
        loop {
            if term_cond.load(AcqRel) || self.run_once().is_none() {
                return;
            }
        }
    }
}

/// A source of uninitialized buffers. Prefer `NullSource` when possible.
/// 
/// In general, you must be careful to avoid attempting to `drop` uninitialized
/// data as in the buffers yielded by this source. With most `Sample`
/// implementors this should not be a major concern (most are primitive types
/// that lack implementations for `Drop`), but the possiblity must be accounted
/// for because `Sample` is an open typeclass.
pub struct UninitializedSource<F> {
    buffer: Vec<F>
}

impl<F: Sample> UninitializedSource<F> {
    /// Create a new source.
    /// 
    /// The yielded buffers will have `size` items.
    pub fn new(size: uint) -> UninitializedSource<F> {
        let mut buffer = Vec::with_capacity(size);
        unsafe {
            buffer.set_len(size);
        }

        UninitializedSource {
            buffer: buffer
        }
    }

    /// Get a buffer.
    /// 
    /// This function is `unsafe` because the returned slice is not initialized
    /// and must not be read until it is first written to. Otherwise, it
    /// behaves exactly like `Source::next`.
    unsafe fn next<'a>(&'a mut self) -> Option<&'a mut [F]> {
        Some(self.buffer.as_mut_slice())
    }
}

/// A source of buffers containing silence.
pub struct NullSource<F> {
    source: UninitializedSource<F>
}

impl<F: Sample> NullSource<F> {
    /// Create a `NullSource` that generates buffers of `size` samples.
    pub fn new(size: uint) -> NullSource<F> {
        NullSource {
            source: UninitializedSource::new(size)
        }
    }
}

impl<F: Sample> Source<F> for NullSource<F> {
    fn next<'a>(&'a mut self) -> Option<&'a mut [F]> {
        unsafe {
            match self.source.next() {
                Some(buffer) => {
                    for i in range(0, buffer.len()) {
                        buffer.init_elem(i, Zero::zero());
                    }
                    Some(buffer)
                },
                None => None
            }
        }
    }
}
