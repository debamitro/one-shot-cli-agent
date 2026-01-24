#!/usr/bin/env sh

ver=0.1
arch="${ARCH:-aarch64-apple-darwin}"
cargo build --target ${arch} --release
cd target/${arch}/release
zip ../../../codeagent_${ver}_${arch}.zip codeagent