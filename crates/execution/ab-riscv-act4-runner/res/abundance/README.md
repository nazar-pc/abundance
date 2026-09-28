# Abundance Cores

These are representative of the features `ab-riscv-interpreter` is capable of.

## `abundance-rv32i-zve32x-zvbb`

RV32I "core" with only Zve32x and Zvbb (and its Zvkb subset) extensions on top of the base ISA and VLEN=128. M is
also enabled because ACT4 requires it for all vector tests. Other extensions are disabled to avoid re-running mostly
the same tests as `abundance-rv32i-max`.

## `abundance-rv64i-zve32x-zvbb`

RV64I "core" with only Zve32x and Zvbb (and its Zvkb subset) extensions on top of the base ISA and VLEN=128. M is
also enabled because ACT4 requires it for all vector tests. Other extensions are disabled to avoid re-running mostly
the same tests as `abundance-rv64i-max`.

## `abundance-rv32i-max`

This is the "maxed out" "core" with every supported extension enabled for RV32I ISA.

## `abundance-rv64i-max`

This is the "maxed out" "core" with every supported extension enabled for RV64I ISA.

## Shared files

Files that are identical between cores are symlinks: all cores use the same `link.ld`, and all cores of the same XLEN
use the same `rvmodel_macros.h`.
