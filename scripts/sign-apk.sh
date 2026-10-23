#!/usr/bin/env bash
# Sign an unsigned Suzatary APK, OUTSIDE the nix store.
#
# WHY THIS IS NOT A DERIVATION. Everything a derivation touches ends up in
# /nix/store, which is world-readable on this box and copied verbatim to every
# substituter it is pushed to. A signing key there is a signing key published.
# So `nix build .#apk-unsigned` (or `.#apk-profiled-unsigned`, impure only to
# read the profile SUZATARY_APK_PROFILE names) is reproducible for its profile
# and produces an APK nobody can install, and this — the one impure step — turns it into one they
# can, from a keystore that lives in secretspec (on the box) or in a GitHub
# Actions secret (in CI) and in neither case in git.
#
# WHY THE KEY CAN NEVER BE ROTATED CASUALLY. Android identifies an app by
# (package name, signing certificate). An update signed with a different key is
# not an update, it is a stranger: the installer refuses it and the only way
# forward is uninstalling, which takes the app's localStorage — the offline
# fold — with it. Losing this keystore therefore costs a reinstall on every
# device, forever. Back it up with the rest of the vault.
#
# Usage:  sign-apk.sh <unsigned.apk> <signed.apk>
# Needs:  apksigner on PATH (`nix run .#apk-sign -- …` supplies it), and
#         SUZATARY_KEYSTORE + SUZATARY_KEYSTORE_PASSWORD in the environment.
#
# ONE-TIME KEY CREATION, in a plain terminal, never in an agent session:
#
#   keytool -genkeypair -v \
#     -keystore ~/suzatary-release.p12 -storetype PKCS12 \
#     -alias suzatary -keyalg RSA -keysize 4096 -validity 10000 \
#     -dname "CN=Suzatary, O=bmabsout, C=PT"
#   secretspec set ANDROID_KEYSTORE_PASSWORD      # the password keytool asked for
#   base64 -w0 ~/suzatary-release.p12 | secretspec set ANDROID_KEYSTORE_B64
#
# RSA is not incidental: apksigner's RSA (PKCS#1 v1.5) signatures are
# deterministic, so re-signing an unchanged APK yields an identical file, while
# ECDSA would produce a different one every run and make "did anything change?"
# unanswerable from the artefact. The change gate reads the unsigned hash
# anyway, but a deterministic signature is what lets a human check the same way.
set -euo pipefail

IN="${1:?usage: sign-apk.sh <unsigned.apk> <signed.apk>}"
OUT="${2:?usage: sign-apk.sh <unsigned.apk> <signed.apk>}"

: "${SUZATARY_KEYSTORE:?sign-apk: SUZATARY_KEYSTORE is unset (path to the PKCS12 keystore)}"
: "${SUZATARY_KEYSTORE_PASSWORD:?sign-apk: SUZATARY_KEYSTORE_PASSWORD is unset}"

if [ ! -r "$SUZATARY_KEYSTORE" ]; then
  echo "sign-apk: cannot read keystore at $SUZATARY_KEYSTORE" >&2
  exit 1
fi

# --ks-pass/--key-pass read the environment rather than the command line: an
# argv is visible to every process on the box, and this script is invoked by a
# CI runner and by a shell whose history is kept.
#
# The signature schemes are left to apksigner, which picks them from
# --min-sdk-version: at 26 that is v2 + v3 and NOT v1, the legacy JAR signature
# no installer this app can run on still reads. Measured, not assumed — the
# verify below prints which schemes actually applied, and asking for v1
# explicitly at this min-sdk does not produce one.
apksigner sign \
  --ks "$SUZATARY_KEYSTORE" \
  --ks-pass env:SUZATARY_KEYSTORE_PASSWORD \
  --key-pass env:SUZATARY_KEYSTORE_PASSWORD \
  --min-sdk-version 26 \
  --out "$OUT" \
  "$IN"

# Verifying what we just wrote is not ceremony: apksigner will happily produce a
# file that a phone then refuses, and the certificate digests printed here are
# what tells a human whether this is the SAME key as the release they have
# installed. A mismatch is the one failure that cannot be fixed by reinstalling
# over the top.
apksigner verify --min-sdk-version 26 --print-certs --verbose "$OUT"
