#!/usr/bin/env bash
set -euo pipefail

version=${1:?Release version required}
revision=${2:?Commit required}
repo=${GITHUB_REPOSITORY:?GITHUB_REPOSITORY required}
cat dist/*.sha256 | LC_ALL=C sort >dist/SHA256SUMS

if [[ "$version" == preview-release ]]; then
  git tag --force preview-release "$revision"
  git push --force origin refs/tags/preview-release
  existing=$(gh api --paginate "repos/$repo/releases?per_page=100" \
    --jq '.[] | select(.tag_name == "preview-release") | .id')
  notes="Rolling preview built from $revision. These assets are replaced after successful main-branch builds. Use a versioned release for reproducible installations."
  if [[ -n "$existing" ]]; then
    gh release edit preview-release --repo "$repo" --title "Preview release" \
      --notes "$notes" --prerelease --latest=false
    gh release upload preview-release dist/* --repo "$repo" --clobber
  else
    gh release create preview-release dist/* --repo "$repo" --verify-tag \
      --title "Preview release" --notes "$notes" --prerelease --latest=false
  fi
else
  gh release create "$version" dist/* --repo "$repo" --verify-tag \
    --title "$version" --generate-notes --latest
fi
