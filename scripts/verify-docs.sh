#!/bin/sh
set -u

failures=0

fail() {
  echo "FAILURE: $1"
  failures=$((failures + 1))
}

for file in README.md GOALS.md AGENTS.md CLAUDE.md .github/pull_request_template.md; do
  [ -s "$file" ] || fail "$file has no text"
done

grep -qx '@AGENTS.md' CLAUDE.md 2>/dev/null || fail "CLAUDE.md does not import AGENTS.md"

documents=$(git ls-files --cached --others --exclude-standard '*.md')

for file in $documents; do
  if grep -q '{{' "$file"; then
    fail "$file contains a template field"
  fi
  directory=$(dirname "$file")
  for target in $(grep -oE '\]\([^)]+\)' "$file" | sed -e 's/^](//' -e 's/)$//' -e 's/#.*$//'); do
    case "$target" in
      http://* | https://* | '') ;;
      *) [ -e "$directory/$target" ] || fail "$file has a link to $target, but there is no file at that path" ;;
    esac
  done
done

expected=1
for id in $(sed -n 's/^### D\([0-9][0-9]*\) (.*$/\1/p' AGENTS.md); do
  [ "$id" -eq "$expected" ] || fail "AGENTS.md has D$id, but D$expected is the next ID"
  expected=$((expected + 1))
done
[ "$expected" -gt 1 ] || fail "AGENTS.md has no decisions"

if [ "$failures" -gt 0 ]; then
  echo "Document check failures: $failures"
  exit 1
fi
echo "All document checks are complete without failures"
