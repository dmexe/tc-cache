#!/bin/bash

set -ex

export TARGET="x86_64-unknown-linux-musl"
export CARGO_ARGS="--color always --target=x86_64-unknown-linux-musl"

cargo fetch                                                              
cargo build ${CARGO_ARGS} --tests                                       
cargo test ${CARGO_ARGS} -- --nocapture                                 
cargo build ${CARGO_ARGS} --bin main --release                          
upx target/${TARGET}/release/main                                       
mkdir -p target/artifacts                                                
cp target/${TARGET}/release/main target/artifacts/tc-cache-$(uname)-$(uname -m)