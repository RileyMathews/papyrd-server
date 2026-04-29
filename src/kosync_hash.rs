const SAMPLE_STEP_BYTES: usize = 1024;
const SAMPLE_SIZE_BYTES: usize = 1024;

pub fn partial_md5(bytes: &[u8]) -> String {
    let mut sampled = Vec::new();

    for offset in sample_offsets() {
        if offset >= bytes.len() {
            break;
        }

        let end = bytes.len().min(offset + SAMPLE_SIZE_BYTES);
        sampled.extend_from_slice(&bytes[offset..end]);
    }

    format!("{:x}", md5::compute(sampled))
}

fn sample_offsets() -> impl Iterator<Item = usize> {
    (-1..=10).map(|index| {
        if index < 0 {
            0
        } else {
            SAMPLE_STEP_BYTES << (2 * index)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{SAMPLE_SIZE_BYTES, partial_md5, sample_offsets};

    #[test]
    fn sample_offsets_match_koreader_partial_md5() {
        assert_eq!(
            sample_offsets().collect::<Vec<_>>(),
            vec![
                0, 1024, 4096, 16384, 65536, 262144, 1048576, 4194304, 16777216, 67108864,
                268435456, 1073741824,
            ]
        );
    }

    #[test]
    fn partial_md5_hashes_only_sampled_bytes() {
        let bytes = (0..20000).map(|value| value as u8).collect::<Vec<_>>();
        let mut sampled = Vec::new();

        for offset in [0, 1024, 4096, 16384] {
            sampled.extend_from_slice(&bytes[offset..offset + SAMPLE_SIZE_BYTES]);
        }

        assert_eq!(partial_md5(&bytes), format!("{:x}", md5::compute(sampled)));
    }
}
