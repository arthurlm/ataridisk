#!/bin/bash
set -ex

mkdir -p data
echo   > data/TEST.TXT "This is a test file"
TZ=utc touch -m data/TEST.TXT -t 202108011430
