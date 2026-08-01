#!/usr/bin/env bash
set -euo pipefail

rustup toolchain install 1.95.0 --profile minimal
cargo +1.95.0 install \
  --git https://github.com/petehunt/topcoat.git \
  --rev eb9c1ee1298f5674dfd5ee2534216499183eaa61 \
  topcoat-cli \
  --locked
RUSTUP_TOOLCHAIN=1.95.0 topcoat asset bundle --release --bin index
