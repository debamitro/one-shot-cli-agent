#!/usr/bin/env sh

ver=0.5
arch="${ARCH:-aarch64-apple-darwin}"
cargo build --target ${arch} --release
cd target/${arch}/release

if [[ -n "$CERTIFICATE" ]]; then
  codesign --options runtime --force -s "${CERTIFICATE}" codeagent
fi

zipfilename="codeagent_${ver}_${arch}.zip"
zip ../../../${zipfilename} codeagent
cd -

[[ -n "$APPLE_ID" ]] || exit
[[ -n "$APP_PASSWORD" ]] || exit
[[ -n "$TEAM_ID" ]] || exit

xcrun notarytool submit ${zipfilename} --wait --apple-id ${APPLE_ID} --password ${APP_PASSWORD} --team-id ${TEAM_ID}

