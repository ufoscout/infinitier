
# Run all tests
[group('test')]
test test_name="":
  cargo test {{test_name}}
  # cargo test {{test_name}} --all-features

# Delete all insta snapshots and regenerate them
[group('test')]
insta_recreate test_name="":
  find . -name "*.snap" -type f -delete
  find . -name "*.snap.new" -type f -delete
  INSTA_UPDATE=always cargo test

