# ab-riscv-act4-runner

Runner for the RISC-V Architectural Certification Tests (ACTs), specifically the ACT4.

`res` directory contains the definition of the cores and Dockerfile ([temporarily]) for building test ELFs that can then
be run by the `ab-riscv-act4-runner`.

[temporarily]: https://github.com/riscv/riscv-arch-test/pull/1161

The workflow is generally the following:

```bash
# Clone ACT4 repo
git clone --depth 1 --branch cert-docs-2026-04-01-17-37-00ca3c9 https://github.com/riscv/riscv-arch-test res/riscv-arch-test
# Build without context since it is not needed (takes a few minutes)
docker build -t riscv-act4 - < res/Dockerfile
# Takes a few minutes, if you plan to call it repeatedly then remove `make` and call it from within instead
docker run -it --rm --privileged \
    -v ./res/riscv-arch-test:/mnt \
    -v ./res/abundance:/mnt/config/abundance:ro \
    -e CONFIG_FILES=config/abundance/abundance-rv64i-max/test_config.yaml \
    riscv-act4 \
    make
# Run generated test ELFs against the interpreter
cargo run -- res/riscv-arch-test/work/abundance-rv64i-max
```
