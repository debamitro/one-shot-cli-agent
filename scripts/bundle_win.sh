#!/usr/bin/env sh

ver=0.1

cargo build --target x86_64-pc-windows-gnu --release
cd target/x86_64-pc-windows-gnu/release
zip ../../../codeagent_${ver}_win.zip codeagent.exe