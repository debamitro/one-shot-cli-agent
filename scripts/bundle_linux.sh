#!/usr/bin/env sh

ver=0.7
arch="${ARCH:-x86_64-unknown-linux-gnu}"
cargo zigbuild --target ${arch} --release
cd target/${arch}/release
zip ../../../codeagent_${ver}_linux_${arch%%-*}.zip codeagent
