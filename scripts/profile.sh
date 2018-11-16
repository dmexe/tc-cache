#!/bin/bash

set -ex
set -o pipefail

export RUST_LOG=info

cache="./target/release/main -H tmp/work"
prefix=tmp/prefix
dir=~/.gradle/caches
pull="${cache} pull ${dir} -p ${prefix}"

rm -rf ${prefix}

dtrace -c "${pull}" -o out.stacks -n 'profile-997 /execname == "main"/ { @[ustack(100)] = count(); }'

rm -f pretty-graph.svg
../FlameGraph/stackcollapse.pl out.stacks | ../FlameGraph/flamegraph.pl > pretty-graph.svg
