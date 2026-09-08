#!/usr/bin/env bash
# Rebuilds every guest program and refreshes the prebuilt binaries the examples embed.
#
# Each program is compiled for the smallest target that has the extensions it uses, which is also what the
# corresponding example composes its instruction set out of. Every combination gets its own target directory, otherwise
# switching between them rebuilds `core` every time.

set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")"

target_dir="../../../../../target"

# Usage: build <binary> <target> <prebuilt name> [target features]
build() {
    local binary=$1 target=$2 name=$3 features=${4:-}

    RUSTFLAGS="${features:+-C target-feature=$features}" \
        cargo build --release -Z build-std=core --target "$target" \
        --target-dir "$target_dir/guests-$name" --bin "$binary"

    install -m 644 "$target_dir/guests-$name/$target/release/$binary" "../prebuilt/$name.elf"
}

build hello riscv32i-unknown-none-elf hello
build checksum riscv64imac-unknown-none-elf checksum
build vector-sum riscv64im-unknown-none-elf vector-sum +zve64x,+zvl128b
build dot-product riscv64im-unknown-none-elf dot-product +zve64x,+zvl128b
