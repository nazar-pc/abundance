//! `GenericBlock` can't be implemented outside `ab_core_primitives`. Code generic over blocks
//! relies on the header corresponding to the body, which their constructors ensure.

use ab_core_primitives::block::GenericBlock;
use ab_core_primitives::block::body::BeaconChainBody;
use ab_core_primitives::block::header::BeaconChainHeader;
use ab_core_primitives::shard::RealShardKind;

#[derive(Debug, Clone)]
struct CustomBlock<'a> {
    header: BeaconChainHeader<'a>,
    body: BeaconChainBody<'a>,
}

impl<'a> GenericBlock<'a> for CustomBlock<'a> {
    const SHARD_KIND: RealShardKind = RealShardKind::BeaconChain;

    type Header = BeaconChainHeader<'a>;
    type Body = BeaconChainBody<'a>;

    fn header(&self) -> &Self::Header {
        &self.header
    }

    fn body(&self) -> &Self::Body {
        &self.body
    }
}

fn main() {}
