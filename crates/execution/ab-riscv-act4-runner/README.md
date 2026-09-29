# ab-riscv-act4-runner

Runner for the RISC-V Architectural Certification Tests (ACTs), specifically the ACT4.

`res` directory contains the definition of the cores for building test ELFs that can then be run by the
`ab-riscv-act4-runner`.

The workflow is generally the following:

```bash
# Joined with spaces below, ACT4's Makefile doesn't handle newlines
CONFIG_FILES=(
    config/abundance/abundance-rv32i-zve32x-zvbb/test_config.yaml
    config/abundance/abundance-rv64i-zve32x-zvbb/test_config.yaml
    config/abundance/abundance-rv32i-max/test_config.yaml
    config/abundance/abundance-rv64i-max/test_config.yaml
)
# Takes a few minutes, if you plan to call it repeatedly then remove `make` and call it from within instead
docker run -it --rm --privileged \
    -v ./res/abundance:/mnt/config/abundance:ro \
    -e CONFIG_FILES="${CONFIG_FILES[*]}" \
    -e EXCLUDE_EXTENSIONS="InterruptsSm,ZawrsSm" \
    ghcr.io/riscv/act4:4.1.0 \
    make
# Run generated test ELFs against the interpreter, the first argument is the name of the core without `abundance-`
# prefix
cargo run -- rv32i-zve32x-zvbb res/riscv-arch-test/work/abundance-rv32i-zve32x-zvbb
cargo run -- rv64i-zve32x-zvbb res/riscv-arch-test/work/abundance-rv64i-zve32x-zvbb
cargo run -- rv32i-max res/riscv-arch-test/work/abundance-rv32i-max
cargo run -- rv64i-max res/riscv-arch-test/work/abundance-rv64i-max
```
