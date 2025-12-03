use ab_core_primitives::address::Address;

/// Parse an address from a string while ignoring the human-readable part
pub(in super::super) fn parse_reward_address(s: &str) -> Result<Address, &'static str> {
    let (_short_hrp, address) = Address::parse(s).ok_or("Invalid reward address")?;
    Ok(address)
}
