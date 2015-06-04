use num::Complex;

pub struct FrequencyData<S> {
    source: S,
    buckets: Arc<RwLock<Vec<f64>>>,
    cplx_in: Vec<Complex>,
    cplx_out: Vec<Complex>
}

impl<S> FrequencyData<S> {
    pub fn new(source: S, nbuckets: usize) -> FrequencyData {
        FrequencyData {
            source: source,
            buckets: Vec::with_capacity(nbuckets),
            cplx_in: Vec::new(),
            cplx_out: Vec::new()
        }
    }

    pub fn get_buckets(&self) -> &RwLock {
        &*self.buckets
    }
}

impl MonoSource<F> for FrequencyData<S> where S: MonoSource<Output=F> {
    fn next<'a>(&'a mut self) -> Option<&'a mut [S::Output]> {
        let samples = match self.source.next() {
            Some(s) => s,
            None => return None
        };

        // Input samples convert to complex for fftw
        self.cplx_in.empty();
        self.cplx_in.extend(samples.iter().map(|s| Complex::new(s.to_float::<f64>(), 0)));

        // Output samples initially zero
        // TODO we can save some cycles by being uninitialized, which might turn out to
        // be safe in all cases (even if Complex implements Drop).
        self.cplx_out.empty();
        self.cplx_out.extend(iter::repeat(Complex::new(0, 0)).taken(samples.len()));

        // Do the FFT and push into buckets
        fftw3::c2c_1d(&input[..], &mut output[..], true).unwrap();
        {
            let mut buckets = self.buckets.write().unwrap();
            buckets.empty();
            buckets.extend(self.cplx_out.iter().map(|e| e.re));
        }

        Some(samples)
    }
}
