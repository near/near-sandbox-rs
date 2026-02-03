#!/usr/bin/env bash
set -e
# Run all examples
for example in $(find . -name "*.rs" -type f); do
    example_name=$(basename $example .rs)

    echo "--------------------------------"
    echo "Running $example_name"
    echo "--------------------------------"

    if [ "$example_name" = "singleton_sandbox" ]; then
        CI=true cargo test --release --example $example_name --features singleton_cleanup -- --no-capture
    else
        CI=true cargo run --release --example $example_name
    fi

    echo "--------------------------------"
done
