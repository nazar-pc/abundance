# riscv-arch-test ACT4 build environment
# https://github.com/riscv/riscv-arch-test (branch: act4)
#
# Includes:
#   - riscv-none-elf-gcc   (xPack GNU RISC-V Embedded GCC)
#   - mise                 (tool manager; installs uv/Python and Ruby automatically)
#   - sail-riscv           (RISC-V reference model)
#
# Usage:
#   docker build -t riscv-act4 .
#   # Create work directory for outputs first, otherwise it will not be owned by the current user
#   mkdir work
#   # Start container if the user is typical 1000:1000
#   docker run -it --rm -v ./<vendor>:/act4/config/cores/<vendor>:ro -v ./work:/act4/work riscv-act4
#   # Or if your user does not have ID 1000
#   docker run -it --rm -u $(id -u):$(id -g) -v ./<vendor>:/act4/config/cores/<vendor>:ro -v ./work:/act4/work riscv-act4
#
#   Inside the container with the pre-cloned and ready to use repo root is at /act4; run:
#     CONFIG_FILES=config/cores/<vendor>/<dut>/test_config.yaml make

# Global ARGs - declared before any FROM so they are overridable across all stages.
# Each stage that uses them must redeclare them with a bare ARG (no default) to bring them into scope.
ARG RISCV_TOOLCHAIN_PREFIX=/opt/riscv
ARG SAIL_VERSION=0.13.1
ARG XPACK_GCC_VERSION=15.2.0-1.1

# Stage 1: install mise, then use it to install uv (Python) and Ruby, and pre-install the riscv-unified-db Bundler gem,
# and pre-download all Python dependencies via uv sync.
#
# mise reads .mise.toml from the repo to pin exact tool versions.
# We pre-warm both the gem cache and the uv virtualenv here so the first `make` inside the container doesn't need
# network access for Ruby or Python deps.
#
# HOME=/home/shared is used so all caches (mise, uv, udb, etc.) land in a single directory that is copied to the final
# image.
FROM ubuntu:26.04@sha256:3131b4cc82a783df6c9df078f86e01819a13594b865c2cad47bd1bca2b7063bb AS mise-fetcher

ARG DEBIAN_FRONTEND=noninteractive

# mise needs curl + ca-certificates to download tools, build-essential is needed for installation of Ruby gems.
RUN apt-get update && apt-get install -y --no-install-recommends \
  ca-certificates \
  curl \
  build-essential \
  && rm -rf /var/lib/apt/lists/*

ENV MISE_YES=1 \
  HOME=/home/shared \
  PATH="/home/shared/.local/bin:${PATH}"

RUN mkdir -p /home/shared

# Install mise itself into ~/.local/bin
RUN curl -fsSL https://mise.jdx.dev/install.sh | sh

# mise and bundler need .mise.toml and the Gemfile(s); uv needs .python-version / pyproject.toml / uv.lock.
COPY \
  .mise.toml \
  framework/src/act/data/Gemfile framework/src/act/data/Gemfile.lock \
  .python-version pyproject.toml uv.lock README.md \
  /act4/
# Copy files of other workspace members
COPY framework/pyproject.toml /act4/framework/pyproject.toml
COPY framework/src/act/__init__.py /act4/framework/src/act/__init__.py
COPY generators/coverage/pyproject.toml /act4/generators/coverage/pyproject.toml
COPY generators/coverage/src/covergroupgen/__init__.py /act4/generators/coverage/src/covergroupgen/__init__.py
COPY generators/testgen/pyproject.toml /act4/generators/testgen/pyproject.toml
COPY generators/testgen/src/testgen/__init__.py /act4/generators/testgen/src/testgen/__init__.py

# Install uv and Ruby at the versions pinned in .mise.toml
# Pre-install the riscv-unified-db gem into the mise-managed Ruby's gem dir so `bundle install` is a no-op at runtime.
# Pre-download all Python dependencies so `uv sync` is a no-op at runtime.
# The final chmod runs in the same layer so the whole tree is not duplicated into a second one.
RUN cd /act4 \
  && mise install \
  && mise exec -- bundle install \
  && mise exec -- uv sync \
  && chmod -R 777 /act4 /home/shared

# Stage 2: fetch riscv-none-elf-gcc (xPack GNU RISC-V Embedded GCC) via xpm.
#
# xpm resolves the host arch itself; under buildx this stage runs emulated as TARGETARCH, so no
# manual arch-name mapping is needed here (contrast with the sail-riscv curl fetch below, which
# needs one because it is a raw tar.gz per-arch download).
FROM ubuntu:26.04@sha256:3131b4cc82a783df6c9df078f86e01819a13594b865c2cad47bd1bca2b7063bb AS xpack-fetcher

ARG DEBIAN_FRONTEND=noninteractive
ARG RISCV_TOOLCHAIN_PREFIX
ARG XPACK_GCC_VERSION

# nodejs/npm from the stock archive are old, but xpm itself has no meaningful version
# sensitivity here; it is only used once, non-interactively, to pull a binary release archive.
RUN apt-get update && apt-get install -y --no-install-recommends \
  ca-certificates \
  nodejs \
  npm \
  && rm -rf /var/lib/apt/lists/*

ENV XPACKS_STORE_FOLDER=/opt/xpacks-store \
  XPACKS_CACHE_FOLDER=/opt/xpacks-cache

RUN npm install --global xpm@0.23.2 \
  && xpm install --global "@xpack-dev-tools/riscv-none-elf-gcc@${XPACK_GCC_VERSION}"

# xpm lays the extracted toolchain out as
# ${XPACKS_STORE_FOLDER}/@xpack-dev-tools/riscv-none-elf-gcc/<version>/.content
# It is moved (not symlinked) to RISCV_TOOLCHAIN_PREFIX because COPY --from in the next stage
# copies symlinks as-is rather than dereferencing them; a symlink pointing back into
# XPACKS_STORE_FOLDER would be copied as a dangling link once that folder is left behind.
RUN toolchain_dir="$(find "${XPACKS_STORE_FOLDER}/@xpack-dev-tools/riscv-none-elf-gcc" -mindepth 1 -maxdepth 1 -type d)" \
  && mv "${toolchain_dir}/.content" "${RISCV_TOOLCHAIN_PREFIX}"

# Stage 3: final runtime image
FROM ubuntu:26.04@sha256:3131b4cc82a783df6c9df078f86e01819a13594b865c2cad47bd1bca2b7063bb AS act4-build

LABEL description="RISC-V Architectural Certification Tests (ACT4) build env"
LABEL org.opencontainers.image.source="https://github.com/riscv/riscv-arch-test"

ARG DEBIAN_FRONTEND=noninteractive
ARG TZ=UTC
ARG RISCV_TOOLCHAIN_PREFIX
ARG SAIL_VERSION
ARG TARGETARCH

ENV TZ="${TZ}" \
  SAIL_PREFIX=/opt/sail \
  MISE_YES=1 \
  HOME=/home/shared
ENV PATH="${RISCV_TOOLCHAIN_PREFIX}/bin:${SAIL_PREFIX}/bin:/home/shared/.local/bin:${PATH}"

# Runtime packages:
#   - make, ca-certificates: drive the ACT4 Makefile
#   - curl: required to fetch the sail-riscv release archive
RUN apt-get update && apt-get install -y --no-install-recommends \
  ca-certificates \
  curl \
  make \
  && rm -rf /var/lib/apt/lists/*

# Fetch and install sail-riscv directly into the final image
RUN case "${TARGETARCH}" in \
    amd64) SAIL_ARCH=x86_64 ;; \
    arm64) SAIL_ARCH=aarch64 ;; \
    *) echo "unsupported architecture: ${TARGETARCH}" >&2; exit 1 ;; \
  esac \
  && mkdir -p "${SAIL_PREFIX}" \
  && curl --location --fail \
  "https://github.com/riscv/sail-riscv/releases/download/${SAIL_VERSION}/sail-riscv-Linux-${SAIL_ARCH}.tar.gz" \
  | tar xz --directory="${SAIL_PREFIX}" --strip-components=1

COPY --from=xpack-fetcher ${RISCV_TOOLCHAIN_PREFIX} ${RISCV_TOOLCHAIN_PREFIX}
COPY --from=mise-fetcher /home/shared /home/shared

# Smoke-test: verify all the binaries landed correctly (runs as root during build, before USER switch)
RUN riscv-none-elf-gcc --version \
  && sail_riscv_sim --version \
  && mise --version

WORKDIR /act4

CMD ["/bin/bash"]

FROM act4-build AS act4

COPY --from=mise-fetcher --chmod=777 /act4/.venv /act4/.venv
COPY --chmod=777 . /act4

# TODO: `make vector-tests` is only needed temporarily before vector tests are enabled by default
# make re-syncs the venv as root, so the fixup has to run after it; restricting it to inodes that
# are actually missing bits keeps the copy-up (and therefore the layer) down to what uv rewrote
RUN make tests \
  && make vector-tests \
  && find tests ! -perm -0777 -exec chmod 0777 {} + \
  && find /act4/.venv ! -perm -0777 -exec chmod 0777 {} +

# Default user with ID 1000 matching typical desktop installation
USER ubuntu
