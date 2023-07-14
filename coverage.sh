#!/bin/bash

rm -rf coverage
mkdir -p coverage

LLVM_PROFILE_FILE='coverage/cargo-test-%p-%m.profraw' cargo test --profile=test-coverage
grcov . --binary-path ./target/test-coverage/deps/ -s . -t html --branch --ignore-not-existing --ignore '../*' --ignore "/*" -o coverage/html
grcov . --binary-path ./target/test-coverage/deps/ -s . -t lcov --branch --ignore-not-existing --ignore '../*' --ignore "/*" -o coverage/lcov.info
