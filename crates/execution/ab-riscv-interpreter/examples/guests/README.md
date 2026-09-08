# Guest programs

Programs that the examples of `ab-riscv-interpreter` execute. They are ordinary Rust compiled for bare-metal RISC-V
targets, deliberately tiny, and their compiled form is committed under `../prebuilt/` so that running an example needs
nothing but this repository.

This is not a member of the repository workspace: `#![no_main]` binaries do not link for the host, and different
programs need different target features, which are per-invocation. The pinned toolchain is still used through
`rust-toolchain.toml` in the repository root, and `-Z build-std=core` builds `core` from the `rust-src` component it
installs, so no additional target has to be installed with `rustup`.

`#![no_std]` is not a choice: a bare-metal RISC-V target has no standard library. `#![no_main]` follows from it, since
`fn main` is a part of that standard library. The entry point has to be called `_start` because that is the symbol the
linker looks for; with anything else the linker warns and leaves the entry point in the ELF header set to zero, which is
where the examples read it from.

Each program is self-contained and has no dependencies, and the example that runs it computes the expected result on the
host independently.

## Rebuilding

```bash
./build.sh
```

Each program is compiled for the smallest target that has the extensions it uses, which is also what the corresponding
example composes its instruction set out of:

| Program       | Target                         | Target features    |
|---------------|--------------------------------|--------------------|
| `hello`       | `riscv32i-unknown-none-elf`    |                    |
| `checksum`    | `riscv64imac-unknown-none-elf` |                    |
| `vector-sum`  | `riscv64im-unknown-none-elf`   | `+zve64x,+zvl128b` |
| `dot-product` | `riscv64im-unknown-none-elf`   | `+zve64x,+zvl128b` |

## Calling convention

The examples call the guest the way any RISC-V caller would: arguments in `a0`, `a1`, ..., result in `a0`, stack pointer
at the top of guest memory, and `ra` pointing at an address outside the program which the interpreter is configured to
treat as "execution finished".

Binaries are linked at the default address the linker picks for this target, `0x10000`, which is also where the examples
place the base of guest memory. No linker script is involved.
