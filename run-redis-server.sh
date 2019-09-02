#!/bin/bash

if [ "$1" == "release" ]; then
    redis-server --loadmodule ./target/release/libspo2.dylib
else
    redis-server --loadmodule ./target/debug/libspo2.dylib
fi
