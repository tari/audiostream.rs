use std::str::FromStr;

pub use self::innards::Feature;

#[cfg(target_arch = "x86_64")]
pub use self::innards::Feature::{Baseline, MMX, SSE, SSE2, SSE3, SSSE3, SSE41,
                                 SSE42, AVX, AVX2};
#[cfg(target_arch = "arm")]
pub use self::innards::Feature::{Baseline, NEON};

use std::os;

lazy_static!{
    static ref FEATURES_OVERRIDE: (Vec<Feature>, Vec<Feature>) = parse_env_overrides();
}

fn parse_env_overrides() -> (Vec<Feature>, Vec<Feature>) {
    let mut blacklist = Vec::<Feature>::new();
    let mut whitelist = Vec::<Feature>::new();

    let eo = match os::getenv("CPU_FEATURES_OVERRIDE") {
        None => {
            return (blacklist, whitelist);
        }
        Some(s) => s
    };

    let plusminus: &[_] = &['+', '-'];
    for feature_spec in eo.as_slice().split(',') {
        let name = feature_spec.trim_left_chars(plusminus);
        let feature: Feature = match FromStr::from_str(name) {
            Some(f) => f,
            None => {
                error!("Invalid CPU feature: {}", name);
                continue;
            }
        };

        (match feature_spec.char_at(0) {
            '+' => &mut whitelist,
            '-' => &mut blacklist,
            c => {
                error!("CPU feature overrides must begin with '+' or '-', not '{}'", c);
                continue;
            }
        }).push(feature);
    }

    (blacklist, whitelist)
}

/// Returns `true` if the CPU supports `feature`.
///
/// ## User overrides
///
/// Setting `CPU_FEATURES_OVERRIDE` in the environment allows automatic feature detection to be
/// overridden in both a positive and negative fashion. The variable should be a comma seperated
/// list of features, each prefixed with '-' or '+' to disable or enable the feature, respectively.
///
/// For example, if the variable's value is "+AVX,-SSE42", AVX will be treated as available and
/// SSE4.2 will be treated as unavailable, no matter what the actual reported CPU support for these
/// features is.
/// 
/// Invalid feature specifications are ignored.
pub fn cpu_supports(feature: Feature) -> bool {
    let (ref blacklist, ref whitelist) = *FEATURES_OVERRIDE;

    if whitelist.iter().any(|f| *f == feature) {
        true
    } else if blacklist.iter().any(|f| *f == feature) {
        false
    } else {
        innards::cpu_supports(feature)
    }
}

#[cfg(target_arch = "x86_64")]
mod innards {
    use self::Feature::*;
    use std::str::FromStr;

    #[deriving(PartialEq, Eq, Show)]
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

    impl FromStr for Feature {
        fn from_str(s: &str) -> Option<Feature> {
            Some(match s {
                "MMX" => MMX,
                "SSE" => SSE,
                "SSE2" => SSE2,
                "SSE3" => SSE3,
                "SSSE3" => SSSE3,
                "SSE41" => SSE41,
                "SSE42" => SSE42,
                "OSXSAVE" => OSXSAVE,
                "AVX" => AVX,
                "AVX2" => AVX2,
                _ => {
                    return None;
                }
            })
        }
    }

    static EBX: uint = 1;
    static ECX: uint = 2;
    static EDX: uint = 3;
    macro_rules! feature(
        ($p_eax:expr : $p_ecx:expr, $reg:expr, $bit:expr) => (
            {
                let mut regs = [0u32, ..4];
                do_cpuid($p_eax, $p_ecx, &mut regs);

                regs[$reg] & (1 << $bit) != 0
            }
        );

        ($reg:expr $bit:expr) => (
            feature!(1:0, $reg, $bit)
        )
    );

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
                && (do_xgetbv(0) & 6 == 6)
                && feature!(ECX 28)
            }
            AVX2 => {
                // Need OS support for AVX and AVX2 feature flag
                cpu_supports(AVX) && feature!(7:0, EBX, 5)
            }
        }
    }

    fn do_cpuid(mut eax: u32, mut ecx: u32, regs: &mut [u32]) {
        let b: u32;
        let d: u32;

        unsafe {
            asm!{
                "cpuid"
                : "+{eax}"(eax), "={ebx}"(b), "+{ecx}"(ecx), "={edx}"(d)
            }
        }

        regs[0] = eax;
        regs[1] = b;
        regs[2] = ecx;
        regs[3] = d;
    }

    fn do_xgetbv(ecx: u32) -> u64 {
        let high: u32;
        let low: u32;

        unsafe {
            asm!{
                "xgetbv"
                : "={edx}"(high), "={eax}"(low)
                : "{ecx}"(ecx)
            }
        }

        (high as u64 << 32) | low as u64
    }
}

#[cfg(target_arch = "arm")]
mod innards {
    use std::from_str::FromStr;

    #[deriving(PartialEq, Eq, Show)]
    pub enum Feature {
        Baseline,
        NEON
    }

    impl FromStr for Feature {
        fn from_str(s: &str) -> Option<Feature> {
            Some(match s {
                "NEON" => NEON,
                _ => {
                    return None;
                }
            })
        }
    }

    pub fn cpu_supports(feature: Feature) -> bool {
        match feature {
            Baseline => true,
            _ => false
        }
    }
}

impl Copy for self::innards::Feature { }
