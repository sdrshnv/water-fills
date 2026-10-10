# Water fills

Tracks 36 oz / 40 oz bottle fills toward the store's 128 oz payment unit.
Dioxus (Rust, WASM) frontend + a Rust Cloudflare Worker + D1 (SQLite) storage, all on the free tier.

- **40 oz / 36 oz**: log a fill. The display shows the total mod 128 and how many payments are owed.
- **Pay!**: enabled only when a payment is owed; marks one payment as paid.
- **Undo**: removes the most recent action (a fill or a Pay). Repeat to walk further back.

State is an append-only event log in D1, so it persists across days and devices.
Every API call needs the `API_KEY` secret as a bearer token; the app asks for it once and remembers it in the browser.

## Layout

| Path | Purpose |
|---|---|
| `shared/` | Types, pure logic, SQL, and view-model. Almost all tests live here. |
| `frontend/` | Dioxus web app (thin; renders `shared::view::ViewModel`). |
| `worker/` | Worker API (`/api/state`, `/api/fill`, `/api/pay`, `/api/undo`) and D1 migration. |
| `scripts/e2e.sh` | curl scenario against `wrangler dev`. |

## Tests

```sh
cargo test -p shared      # unit, scenario, SQL (in-memory SQLite) and property tests
cargo clippy -p shared --all-targets
cargo check --target wasm32-unknown-unknown -p frontend -p worker-api
```

End to end, locally:

```sh
echo 'API_KEY=test-key' > .dev.vars
npx wrangler d1 migrations apply water-fills --local
dx bundle --platform web --release -p frontend
npx wrangler dev                 # then, in another shell, from a fresh DB:
scripts/e2e.sh
```

(`scripts/e2e.sh` expects an empty database; delete `.wrangler/state` to reset.)

## One-time setup and deploy

Prerequisites: `rustup target add wasm32-unknown-unknown`, `cargo install worker-build`, `npm install`, and the `dx` CLI
(version must match the pinned `dioxus` crate, currently 0.7.6).

```sh
npx wrangler login
npx wrangler d1 create water-fills        # paste the printed database_id into wrangler.toml
npx wrangler d1 migrations apply water-fills --remote
npx wrangler secret put API_KEY           # choose your access key
dx bundle --platform web --release -p frontend
npx wrangler deploy
```

Open the `*.workers.dev` URL on your phone, enter the key once, and add the page to your home screen.

## Redeploying

```sh
dx bundle --platform web --release -p frontend && npx wrangler deploy
```
