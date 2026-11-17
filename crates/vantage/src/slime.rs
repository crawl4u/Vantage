//! Bedrock slime chunks. Unlike Java they do not depend on the world seed: a chunk is a slime
//! chunk when the first Mersenne Twister output seeded from its coordinates is a multiple of 10.

/// Whether slimes can spawn below Y 40 in this chunk of the Overworld.
pub fn is_slime_chunk(chunk_x: i32, chunk_z: i32) -> bool {
    let seed = (chunk_x as u32).wrapping_mul(0x1f1f_1f1f) ^ (chunk_z as u32);
    first_output(seed) % 10 == 0
}

/// The first MT19937 output for `seed`. Only the words the first twist reads (0, 1 and 397)
/// are needed, so the state is generated just that far.
pub fn first_output(seed: u32) -> u32 {
    let mut previous = seed;
    let first = seed;
    let mut second = 0;
    let mut middle = 0;
    for index in 1..=397u32 {
        previous = 1_812_433_253u32.wrapping_mul(previous ^ (previous >> 30)).wrapping_add(index);
        if index == 1 {
            second = previous;
        }
        middle = previous;
    }
    let y = (first & 0x8000_0000) | (second & 0x7fff_ffff);
    let mut value = middle ^ (y >> 1) ^ if y & 1 != 0 { 0x9908_b0df } else { 0 };
    value ^= value >> 11;
    value ^= (value << 7) & 0x9d2c_5680;
    value ^= (value << 15) & 0xefc6_0000;
    value ^ (value >> 18)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A full MT19937 to check the shortcut against.
    fn reference(seed: u32) -> u32 {
        let mut mt = [0u32; 624];
        mt[0] = seed;
        for i in 1..624 {
            mt[i] = 1_812_433_253u32.wrapping_mul(mt[i - 1] ^ (mt[i - 1] >> 30)).wrapping_add(i as u32);
        }
        for i in 0..624 {
            let y = (mt[i] & 0x8000_0000) | (mt[(i + 1) % 624] & 0x7fff_ffff);
            mt[i] = mt[(i + 397) % 624] ^ (y >> 1) ^ if y & 1 != 0 { 0x9908_b0df } else { 0 };
        }
        let mut value = mt[0];
        value ^= value >> 11;
        value ^= (value << 7) & 0x9d2c_5680;
        value ^= (value << 15) & 0xefc6_0000;
        value ^ (value >> 18)
    }

    #[test]
    fn matches_the_standard_generator() {
        // The published first output of MT19937 for its default seed.
        assert_eq!(first_output(5489), 3_499_211_612);
        for seed in [0, 1, 42, 0x1f1f_1f1f, u32::MAX, 123_456_789] {
            assert_eq!(first_output(seed), reference(seed));
        }
    }

    #[test]
    fn about_one_chunk_in_ten_is_a_slime_chunk() {
        let count = (-50..50).flat_map(|x| (-50..50).map(move |z| (x, z))).filter(|(x, z)| is_slime_chunk(*x, *z)).count();
        assert!((800..1200).contains(&count), "{count}");
    }
}
