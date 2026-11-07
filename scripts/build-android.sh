#!/bin/sh
set -eu

export JAVA_HOME="${JAVA_HOME:-/opt/homebrew/opt/openjdk@21}"
export ANDROID_HOME="${ANDROID_HOME:-$HOME/Library/Android/sdk}"

generated=apps/android/app/build/generated/uniffi
libraries=apps/android/app/src/main/jniLibs

cargo build --release -p write-ffi
cargo run --release -p write-ffi --bin uniffi-bindgen -- generate \
  --library target/release/libwrite_ffi.dylib \
  --language kotlin \
  --out-dir "$generated"
cargo ndk -t arm64-v8a -t x86_64 -o "$libraries" build --release -p write-ffi
apps/android/gradlew -p apps/android assembleDebug
echo "Built apps/android/app/build/outputs/apk/debug/app-debug.apk"
