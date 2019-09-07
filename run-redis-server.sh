#!/bin/bash

set -e

if [ "$1" == "release" ]; then
    cargo build --release
    redis-server --loadmodule ./target/release/libspo2.dylib
else
    cargo build
    redis-server --loadmodule ./target/debug/libspo2.dylib
fi
