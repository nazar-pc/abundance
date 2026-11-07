//! `GenericOwnedBlock` can't be implemented outside `ab_core_primitives`. Code generic over owned
//! blocks relies on the header corresponding to the body, which their constructors ensure.

use ab_aligned_buffer::SharedAlignedBuffer;
use ab_core_primitives::block::BeaconChainBlock;
use ab_core_primitives::block::body::owned::OwnedBeaconChainBody;
use ab_core_primitives::block::header::owned::OwnedBeaconChainHeader;
use ab_core_primitives::block::owned::{GenericOwnedBlock, OwnedBlock};
use ab_core_primitives::shard::RealShardKind;

#[derive(Debug, Clone)]
struct CustomBlock {
    header: OwnedBeaconChainHeader,
    body: OwnedBeaconChainBody,
}

impl From<CustomBlock> for OwnedBlock {
    fn from(_block: CustomBlock) -> Self {
        unimplemented!()
    }
}

impl GenericOwnedBlock for CustomBlock {
    const SHARD_KIND: RealShardKind = RealShardKind::BeaconChain;

    type Header = OwnedBeaconChainHeader;
    type Body = OwnedBeaconChainBody;
    type Block<'a> = BeaconChainBlock<'a>;

    fn split(self) -> (Self::Header, Self::Body) {
        (self.header, self.body)
    }

    fn header(&self) -> &Self::Header {
        &self.header
    }

    fn body(&self) -> &Self::Body {
        &self.body
    }

    fn from_buffers(_header: SharedAlignedBuffer, _body: SharedAlignedBuffer) -> Option<Self> {
        None
    }

    fn block(&self) -> Self::Block<'_> {
        unimplemented!()
    }
}

fn main() {}
