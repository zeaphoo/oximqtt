#!/bin/bash

set -e

publish() {
    echo ">>> Publishing $1 ..."
    cargo publish --all-features --manifest-path "$1/Cargo.toml"
    echo ">>> $1 published, waiting for index update..."
    sleep 15
}

# oximqtt (contains codec, net, utils, conf)
publish oximqtt

echo ">>> All crates published."
