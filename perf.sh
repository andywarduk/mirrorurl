#!/bin/bash

cargo build --profile=release-with-debug
#perf record --call-graph lbr  target/release-with-debug/mirrorurl $*
perf record --call-graph dwarf,16384 target/release-with-debug/mirrorurl $*
perf report
