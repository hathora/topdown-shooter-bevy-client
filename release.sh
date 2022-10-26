#!/bin/bash

cargo build --release
mkdir -p release
cp ./target/release/topdown-shooter-bevy-client release
cp -r assets release
zip -r release.zip release