#![expect(incomplete_features, reason = "generic_const_*")]
#![feature(
    const_default,
    const_trait_impl,
    derive_const,
    generic_const_args,
    macroless_generic_const_args,
    min_generic_const_args
)]

mod balanced;
#[cfg(feature = "blake3")]
mod blake3;
mod mmr;
mod size_dependent_hasher;
mod sparse;
mod test_hasher;
mod unbalanced;
