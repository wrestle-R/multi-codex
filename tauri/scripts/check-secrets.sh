#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/../.."

if git ls-files | grep -E '(^|/)(auth[^/]*\.json|credentials[^/]*\.json|profiles\.json|[^/]*\.(token|secret|keyring))$'; then
  echo "Tracked credential or app-data filename detected" >&2
  exit 1
fi

openai_prefix='s''k-'
jwt_prefix='e''yJ'
if git grep -n -I -E "${openai_prefix}[A-Za-z0-9_-]{20,}|${jwt_prefix}[A-Za-z0-9_-]{30,}" -- ':!tauri/scripts/check-secrets.sh'; then
  echo "Possible credential value detected in tracked content" >&2
  exit 1
fi

echo "Tracked secret scan passed"
