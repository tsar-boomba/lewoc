use core::ops::Range;

use rand_core::RngCore;

pub fn random_u32_in_range(rng: &mut impl RngCore, range: Range<u32>) -> u32 {
    (rng.next_u32() % (range.end - range.start)) + range.start
}
