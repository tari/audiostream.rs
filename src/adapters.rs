
pub struct MonoFilter<S, F, T> {
    source: S,
    builder: F,
}

struct DummySource<T> {
    data: &mut [T]
}

impl<S, F, T> Source for MonoFilter<S, F, T>
        where T: MonoSource<Output=S::Output>,
              F: FnMut(DummySource) -> T {
    type Output = S::Output;

    fn next<'a>(&'a mut self) -> SourceResult<'a, Self::Output> {
        let buffer = match self.source.next() {
            SourceResult::Buffer(b) => b,
            x => return x
        };

        for channel in buffer.mut_iter() {
            let filter = self.builder(DummySource { data: channel });
            match filter.next() {
                None => SourceResult::EndOfStream,
                Some(b) => *channel = b
            }
        }
        SourceResult::Buffer(buffer)
    }
}
