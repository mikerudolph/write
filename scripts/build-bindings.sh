#!/bin/sh
set -eu

cargo build --release -p write-ffi
cargo run --release -p write-ffi --bin uniffi-bindgen -- generate \
  --library target/release/libwrite_ffi.dylib \
  --language swift \
  --out-dir build/bindings/swift

mkdir -p apps/mac/Sources/write_ffiFFI/include
cp build/bindings/swift/write_ffi.swift apps/mac/Sources/WriteCore/write_ffi.swift
cp build/bindings/swift/write_ffiFFI.h apps/mac/Sources/write_ffiFFI/include/write_ffiFFI.h
cp build/bindings/swift/write_ffiFFI.modulemap apps/mac/Sources/write_ffiFFI/include/module.modulemap
