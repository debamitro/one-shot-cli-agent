#!/usr/bin/env sh

ver=0.4

cargo zigbuild --target x86_64-unknown-linux-gnu --release
cd target/x86_64-unknown-linux-gnu/release
zip ../../../codeagent_${ver}_linux.zip codeagent
