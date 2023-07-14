#!/bin/bash

cargo flamegraph --profile=release-with-debug -- $*
