use ab_core_primitives::address::{Address, ShortHrp};
use ab_core_primitives::shard::ShardIndex;

#[test]
fn format() {
    let test_vectors = [
        // Fully collapsed
        (Address::NULL, &ShortHrp::MAINNET, "abc1-ldnky6"),
        // Shard part fully collapsed and most of the address
        (Address::SYSTEM_CODE, &ShortHrp::MAINNET, "abc1-y-s83stq"),
        // Shard part collapsed partially and address part collapsed fully
        (
            Address::system_address_allocator(ShardIndex::new(1).unwrap()),
            &ShortHrp::MAINNET,
            "abc1s-x5j6vw",
        ),
        // Shard part collapsed partially and several address sections collapsed
        (
            Address::from(u128::from_be_bytes([
                1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 42, 0, 0, 0, 42,
            ])),
            &ShortHrp::MAINNET,
            "abc1qy-4-qqq-qq9g-mzcrt4",
        ),
        // Full length
        (
            Address::from(668134116173567166818323700814383461),
            &ShortHrp::MAINNET,
            "abc1qzq2-mr3r-nsr-wcka-6fk6-sdz-cfv5-ygv5w7",
        ),
        // Max address
        (
            Address::from(u128::MAX),
            &ShortHrp::MAINNET,
            "abc1llll-llll-lll-llll-llll-lll-lllu-9vt6a6",
        ),
    ];

    for (address, hrp, s) in test_vectors {
        assert_eq!(address.format(hrp).as_str(), s, "{address:?}, {hrp:?}");
        assert_eq!(
            Address::parse(s),
            Some((*hrp, address)),
            "{address:?}, {hrp:?}"
        );

        // Corrupted
        {
            let mut s = s.to_string();
            // SAFETY: Keys are known to exist and all bytes are ASCII values
            unsafe {
                s.as_bytes_mut()[5] = b'k';
                s.as_bytes_mut()[6] = b'l';
                s.as_bytes_mut()[7] = b'm';
            }
            assert!(Address::parse(&s).is_none(), "{address:?}, {hrp:?}");
        }
    }
}
