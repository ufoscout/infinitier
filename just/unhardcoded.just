
GEMRB_REPO := "https://github.com/gemrb/gemrb.git"


# Copies the `unhardcoded` folder from the gemrb GitHub repository into ./unhardcoded
[group('unhardcoded')]
update-unhardcoded:
  #!/usr/bin/env bash
  set -euo pipefail
  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' EXIT
  git clone --depth 1 --filter=blob:none --sparse "{{GEMRB_REPO}}" "$tmp"
  git -C "$tmp" sparse-checkout set gemrb/unhardcoded
  rm -rf ./src/core/unhardcoded
  cp -r "$tmp/gemrb/unhardcoded" ./src/core/unhardcoded
  echo "Copied unhardcoded into ./src/core/unhardcoded"
