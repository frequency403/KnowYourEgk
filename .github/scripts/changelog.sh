#!/usr/bin/env bash
# Prints release notes (Markdown) for tag $1: all non-merge commits since
# the previous vX.Y.Z tag, or since the first commit for the first release.
# Usage: changelog.sh <tag> [owner/repo]
set -euo pipefail

tag=$1
repo=${2:-}

prev=$(git describe --tags --abbrev=0 --match 'v[0-9]*.[0-9]*.[0-9]*' "$tag^" 2>/dev/null || true)
if [[ -n "$prev" ]]; then
  range="$prev..$tag"
  echo "## Changes since $prev"
else
  range="$tag"
  echo "## Changes"
fi
echo

git log --no-merges --reverse --pretty='format:- %s (%h)' "$range"
echo
echo

if [[ -n "$prev" && -n "$repo" ]]; then
  echo "**Full diff:** https://github.com/$repo/compare/$prev...$tag"
  echo
fi
