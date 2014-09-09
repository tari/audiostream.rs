pub enum Feature {
    Baseline,
    MMX,
    SSE,
    SSE2,
    SSE3,
    SSSE3,
    SSE41,
    SSE42,
    OSXSAVE,
    AVX,
    AVX2
}

static EBX: uint = 1;
static ECX: uint = 2;
static EDX: uint = 3;
macro_rules! feature(
    ($p_eax:expr : $p_ecx:expr, $reg:expr, $bit:expr) => (
        {
            let mut regs = [0u32, ..4];
            unsafe { do_cpuid($p_eax, $p_ecx, regs.as_mut_ptr()); }

            regs[$reg] & (1 << $bit) != 0
        }
    );

    ($reg:expr $bit:expr) => (
        feature!(1:0, $reg, $bit)
    )
)

pub fn cpu_supports(feature: Feature) -> bool {
    match feature {
        Baseline => true,
        MMX => feature!(EDX 23),
        SSE => feature!(EDX 25),
        SSE2 => feature!(EDX 26),
        SSE3 => feature!(ECX 0),
        SSSE3 => feature!(ECX 9),
        SSE41 => feature!(ECX 19),
        SSE42 => feature!(ECX 20),
        OSXSAVE => feature!(ECX 27),
        AVX => {
            // Requires that OS support for XSAVE be in use and enabled for AVX
            cpu_supports(OSXSAVE)
            && (unsafe { do_xgetbv(0) & 6 } == 6)
            && feature!(ECX 28)
        }
        AVX2 => {
            // Need OS support for AVX and AVX2 feature flag
            cpu_supports(AVX) && feature!(7:0, EBX, 5)
        }
    }
}

// Inline assembly appears to handle this poorly. Easier just to let a C compiler do it.
#[link(name = "cpuid", kind = "static")]
extern "C" {
    fn do_cpuid(eax: u32, ecx: u32, outputs: *mut u32);
    fn do_xgetbv(ecx: u32) -> u64;
}

