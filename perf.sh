#!/bin/bash

cargo build --profile=release-with-debug
perf record --call-graph lbr  target/release-with-debug/mirrorurl $*
perf report
