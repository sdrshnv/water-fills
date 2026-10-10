#!/usr/bin/env bash
# End-to-end check against a running `wrangler dev` (fresh local database).
# Usage: scripts/e2e.sh [base-url] [api-key]
set -euo pipefail

BASE="${1:-http://localhost:8787}"
KEY="${2:-test-key}"
fails=0

# req METHOD PATH [JSON-BODY] -> sets $STATUS and $BODY
req() {
  local method=$1 path=$2 body=${3:-}
  local args=(-s -o /tmp/e2e_body -w '%{http_code}' -X "$method" "$BASE$path" -H "Authorization: Bearer $KEY")
  [[ -n "$body" ]] && args+=(-H 'Content-Type: application/json' -d "$body")
  STATUS=$(curl "${args[@]}")
  BODY=$(cat /tmp/e2e_body)
}

expect() { # description, expected status, expected body (exact, optional)
  local desc=$1 want_status=$2 want_body=${3:-}
  if [[ "$STATUS" != "$want_status" || ( -n "$want_body" && "$BODY" != "$want_body" ) ]]; then
    echo "FAIL: $desc -> got $STATUS $BODY (wanted $want_status ${want_body})"
    fails=$((fails + 1))
  else
    echo "ok:   $desc"
  fi
}

# Auth
STATUS=$(curl -s -o /dev/null -w '%{http_code}' "$BASE/api/state")
expect "no key is rejected" 401
STATUS=$(curl -s -o /dev/null -w '%{http_code}' "$BASE/api/state" -H 'Authorization: Bearer wrong')
expect "wrong key is rejected" 401
STATUS=$(curl -s -o /dev/null -w '%{http_code}' -X POST "$BASE/api/fill" -d '{"oz":40}')
expect "unauthenticated fill is rejected" 401

# Fresh state
req GET /api/state;                         expect "empty state" 200 '{"oz":0,"owed":0,"last":null}'
req POST /api/undo;                         expect "undo on empty" 409
req POST /api/pay;                          expect "pay with nothing owed" 409

# Fills
req POST /api/fill '{"oz":40}';             expect "fill 40" 200 '{"oz":40,"owed":0,"last":{"Fill":40}}'
req POST /api/fill '{"oz":40}';             expect "fill 40" 200 '{"oz":80,"owed":0,"last":{"Fill":40}}'
req POST /api/fill '{"oz":36}';             expect "fill 36" 200 '{"oz":116,"owed":0,"last":{"Fill":36}}'
req POST /api/pay;                          expect "pay at 116 oz" 409
req POST /api/fill '{"oz":37}';             expect "invalid fill 37" 400
req POST /api/fill '{"oz":"x"}';            expect "malformed fill" 400
req GET /api/state;                         expect "state unchanged by rejects" 200 '{"oz":116,"owed":0,"last":{"Fill":36}}'

# Crossing 128
req POST /api/fill '{"oz":40}';             expect "fill 40 crosses 128" 200 '{"oz":28,"owed":1,"last":{"Fill":40}}'
req POST /api/pay;                          expect "pay" 200 '{"oz":28,"owed":0,"last":"Pay"}'
req POST /api/pay;                          expect "second pay" 409

# Undo walks back
req POST /api/undo;                         expect "undo pay" 200 '{"oz":28,"owed":1,"last":{"Fill":40}}'
req POST /api/undo;                         expect "undo boundary fill" 200 '{"oz":116,"owed":0,"last":{"Fill":36}}'
req POST /api/undo;                         expect "undo" 200 '{"oz":80,"owed":0,"last":{"Fill":40}}'
req POST /api/undo;                         expect "undo" 200 '{"oz":40,"owed":0,"last":{"Fill":40}}'
req POST /api/undo;                         expect "undo to empty" 200 '{"oz":0,"owed":0,"last":null}'
req POST /api/undo;                         expect "undo past empty" 409

# Methods / routes
req DELETE /api/state;                      expect "wrong method" 405
req GET /api/nope;                          expect "unknown api route" 404

if [[ $fails -gt 0 ]]; then
  echo "$fails check(s) failed"
  exit 1
fi
echo "all checks passed"
