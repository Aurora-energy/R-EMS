#!/bin/sh
set -eu

# Ensure every workspace member has a manifest and placeholder source so that
# `cargo fetch` succeeds even before the real sources are copied into the build
# context. This keeps dependency caching effective across all service images.
bin_members="bus configd gui logger registry supervisor"

for crate in $bin_members; do
    mkdir -p "services/${crate}/src"
    cat <<'RS' > "services/${crate}/src/main.rs"
fn main() {}
RS
done

mkdir -p services/schemas/src
cat <<'RS' > services/schemas/src/lib.rs
pub fn __placeholder() {}
RS

cat <<'RS' > services/schemas/build.rs
fn main() {}
RS

