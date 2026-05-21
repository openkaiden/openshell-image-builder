#!/bin/bash
set -e

cargo +stable install cargo-llvm-cov --locked
rustup component add llvm-tools-preview --toolchain stable
