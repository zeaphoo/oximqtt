#!/bin/bash

cargo publish --registry crates-io --all-features --manifest-path oximqtt/Cargo.toml

sleep 15

cargo publish --registry crates-io --all-features --manifest-path oximqtt-bin/Cargo.toml

# cargo publish --registry crates-io --all-features --manifest-path oximqtt-macros/Cargo.toml
# cargo publish --registry crates-io --all-features --manifest-path oximqtt-utils/Cargo.toml
# cargo publish --registry crates-io --all-features --manifest-path oximqtt-codec/Cargo.toml
# cargo publish --registry crates-io --all-features --manifest-path oximqtt-net/Cargo.toml
# cargo publish --registry crates-io --all-features --manifest-path oximqtt-conf/Cargo.toml
