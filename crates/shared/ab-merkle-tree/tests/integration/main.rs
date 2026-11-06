#![expect(incomplete_features, reason = "generic_const_*")]
#![feature(
    const_default,
    const_trait_impl,
    derive_const,
    gca_const_items,
    gca_macroless_args,
    gca_min_const_items
)]

mod balanced;
#[cfg(feature = "blake3")]
mod blake3;
mod mmr;
mod size_dependent_hasher;
mod sparse;
mod test_hasher;
mod unbalanced;
