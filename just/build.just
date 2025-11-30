
# Cleans the build artifacts
[group('build')]
clean:
  cargo clean


# Builds the test canister
[group('build')]
build:
  cargo build --release
