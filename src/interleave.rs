// Two-way interleave:
// Given [[T]] where the major order is 2
//
// n x T vectors A and B for each channel
//
// Interleave in LLVM IR:
//  <result> = shufflevector <4 x i32> %A, <4 x i32> %B,
//                      <8 x i32> <i32 0, i32 4, i32 1, i32 5,
//                                 i32 2, i32 6, i32 3, i32 7>

#[cfg(target_arch = "arm")] use std::cmp;
use std::ptr;
use super::cpu;

/*
#[simd]
#[allow(non_camel_case_types)]
/// 64-bit vector
struct i16x4(i16, i16, i16, i16);
*/
#[simd]
#[allow(non_camel_case_types, dead_code)]
/// 128-bit vector
struct i16x8(i16, i16, i16, i16, i16, i16, i16, i16);
#[simd]
#[allow(non_camel_case_types, dead_code)]
/// 256-bit vector
struct i16x16(i16, i16, i16, i16, i16, i16, i16, i16,
              i16, i16, i16, i16, i16, i16, i16, i16);

fn interleave_arbitrary<T: Copy>(channels: &[&[T]], out: &mut [T]) {
    let width = channels.len();
    for (i, p) in out.mut_iter().enumerate() {
        unsafe {
            ptr::write(p as *mut _, channels[i % width][i / width]);
        }
    }
}

/// Types which can be interleaved.
///
/// Interleaving two slices `[a0, a1, a2]` and `[b0, b1, b2]` yields the output slice
/// `[a0, b0, a1, b1, a2, b2]` and so forth. The native format for the library is interleaved, but
/// most input and output formats expect an interleaved stream. This trait is used for those
/// conversions.
pub trait Interleave : Copy {
    /// Interleaves all channels in `input` into output.
    ///
    /// `out`'s contents must not require `drop`ping -- it is expected that the values there on entry
    /// are uninitialized.
    fn interleave(channels: &[&[Self]], out: &mut [Self]) {
        Interleave::validate(channels, out);
        interleave_arbitrary(channels, out);
    }
    /// Convenience method to sanity check parameters.
    ///
    /// Ensures that all channels are the same length and the output slice's length is equal to the
    /// product of the number of channels and the length of each channel.
    ///
    /// This function shouldn't be needed for external users; only for implementations of this
    /// trait.
    fn validate(channels: &[&[Self]], out: &mut [Self]) {
        let len = channels[0].len();
        for channel in channels.iter() {
            assert_eq!(channel.len(), len);
        }
        assert_eq!(len * channels.len(), out.len());
    }

}

#[cfg(target_arch = "x86_64")]
static FEATURES: [cpu::Feature, ..2] = [
    cpu::AVX,
    cpu::Baseline
];
#[cfg(target_arch = "arm")]
static FEATURES: [cpu::Feature, ..2] = [
    cpu::NEON,
    cpu::Baseline
];
fn prioritize_features() -> cpu::Feature {
    for &feature in FEATURES.iter() {
        if cpu::cpu_supports(feature) {
            info!("Detected best CPU feature is {}", feature);
            return feature;
        }
    }
    unreachable!()
}

lazy_static!(
    static ref CPU_BEST_FEATURE: cpu::Feature = prioritize_features();
)

impl Interleave for i16 {
    #[cfg(target_arch = "x86_64")]
    fn interleave(channels: &[&[i16]], out: &mut [i16]) {
        Interleave::validate(channels, out);

        match (*CPU_BEST_FEATURE, channels) {
            (cpu::AVX, [left, right]) => {
                // No particular alignment restrictions here
                unsafe {
                    i16x2_fast_avx(left, right, out);
                }
            }
            (_, channels) => {
                interleave_arbitrary(channels, out)
            }
        }
    }

    #[cfg(target_arch = "arm")]
    fn interleave(channels: &[&[i16]], out: &mut [i16]) {
        Interleave::validate(channels, out);

        match (*CPU_BEST_FEATURE, channels) {
            (cpu::NEON, [left, right]) => {
                // Buffers must have same alignment to even have a chance of
                // permitting 128-bit loads.
                // TODO need to look at alignment for `out` too.
                if (left.as_ptr() as uint) & 7 == (right.as_ptr() as uint) & 7 {
                    i16x2_fast_neon(left, right, out);
                } else {
                    interleave_arbitrary(&[left, right], out);
                }
            }
            (_, channels) => {
                interleave_arbitrary(channels, out);
            }
        }
    }
}

impl Interleave for i8 { }
// i16 optimized
impl Interleave for i32 { }
impl Interleave for f32 { }
impl Interleave for f64 { }

#[cfg(target_arch = "x86_64")]
unsafe fn i16x2_fast_avx(xs: &[i16], ys: &[i16], zs: &mut [i16]) {
    let n = xs.len();
    let a = xs.as_ptr();
    let b = ys.as_ptr();
    let out = zs.as_mut_ptr();

    // Take vectors 8 samples at a time from each channel
    for i in range(0, n / 8) {
        let left: *const i16x8 = (a as *const i16x8).offset(i as int);
        let right: *const i16x8 = (b as *const i16x8).offset(i as int);
        let mixed: *mut i16x16 = (out as *mut i16x16).offset(i as int);

        // vmovdqa would be better, but requires 256-bit memory alignment.
        // Same for vmovaps, but that's more reasonable since it's a 256-bit store.
        asm!{
            "vmovdqu ($0), %xmm0
             vmovdqu ($1), %xmm1
             vpunpckhwd %xmm1, %xmm0, %xmm2
             vpunpcklwd %xmm1, %xmm0, %xmm0
             vinsertf128 $$1, %xmm2, %ymm0, %ymm0
             vmovups %ymm0, ($2)"
            :                                   // Output
            : "r"(left), "r"(right), "r"(mixed) // Input
            : "%ymm0", "%xmm1", "%xmm2"         // Clobbers
        };
    }

    // Non-multiple of 8 tail
    interleave_arbitrary(&[xs.slice_from(n & !7), ys.slice_from(n & !7)],
                         zs.mut_slice_from(2 * (n & !7)));
}

#[cfg(target_arch = "arm")]
fn i16x2_fast_neon(xs: &[i16], ys: &[i16], zs: &mut [i16]) {
    let n = xs.len();

    // Three parts: head to get 128-bit alignment, mid where we can do 128-bit loads and stores,
    // tail for trailing non-multiples of 8.
    let n_head = cmp::max(xs.as_ptr() as uint & 7, xs.len());
    let n_tail = (n - n_head) & 7;
    let n_mid = n - n_head - n_tail;

    if n_head > 0 {
        interleave_arbitrary(&[xs.slice_to(n_head - 1), ys.slice_to(n_head - 1)],
                             zs.mut_slice_to(2 * (n_head - 1)));
    }
    // Now for the vectorization
    {
        unsafe {
            let mut a = xs.slice(n_head, n - n_tail).as_ptr() as *const i16x8;
            let mut b = ys.slice(n_head, n - n_tail).as_ptr() as *const i16x8;
            let mut out = zs.mut_slice(n_head * 2, (n - n_tail) * 2).as_mut_ptr() as *mut i16x8;

            for i in range(0, n_mid / 8) {
                asm!{
                    "vldmia $1!, {Q0}
                     vldm $2!, {Q1}
                     vzip.16 Q0, Q1
                     vstm $0!, {Q0, Q1}"
                    : "+r"(out), "+r"(a), "+r"(b)
                    : 
                    : "Q0", "Q1"
                }
            }
        }
    }
    if n_tail > 0 {
        interleave_arbitrary(&[xs.slice_from(n_head + n_mid), ys.slice_from(n_head + n_mid)],
                             zs.mut_slice_from(2 * (n_head + n_mid)));
    }
}

#[cfg(test)]
mod test {
    extern crate test;
    use self::test::Bencher;
    use super::Interleave;

    #[test]
    fn test_interleave_2x2x1024() {
        let mut a = [0i16, ..1024];
        for (i, p) in a.mut_iter().enumerate() {
            *p = i as i16;
        }
        let b = a;

        let mut i = unsafe {
            ::std::mem::uninitialized::<[i16, ..2048]>()
        };
        Interleave::interleave(&[&a, &b], &mut i);

        for idx in ::std::iter::range(0, i.len() / 2) {
            assert_eq!(i[idx * 2], idx as i16);
            assert_eq!(i[idx * 2 + 1], idx as i16);
        }
    }

    #[bench]
    fn bench_interleave_2x2(bencher: &mut Bencher) {
        let mut a = [0i16, ..2048];
        for (i, p) in a.mut_iter().enumerate() {
            *p = i as i16;
        }
        let mut b = a;

        let mut i = unsafe {
            ::std::mem::uninitialized::<[i16, ..4096]>()
        };

        bencher.iter(|| Interleave::interleave(&[&mut a, &mut b], &mut i));
        bencher.bytes = 4096;
    }
}
