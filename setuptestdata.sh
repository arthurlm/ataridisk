#!/bin/bash
set -ex

mkdir -p data
echo   > data/TEST.TXT "This is a test file"
touch -m data/TEST.TXT -t 202108011430
