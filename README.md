# Chores

A simpler companion to ForkBasket for sharing the work at home. Two adults record their own chores, compare a rolling points balance, and alternate bedtime duties with fair two-night swaps.

Live at **https://chores.fruitiex.org** through the homelab's private HTTPS gateway. Connect to the home network or VPN to use it. First launch walks you through household names, both accounts, and the bedtime starting night; no setup token is needed. Save each account's recovery code in its password manager.

- Configurable chores, points, categories, icons and optional maintenance intervals.
- A rolling last-30-days balance, adjustable to other periods or all time.
- Historical point values, completion notes and dates, and undo with a preserved history entry.
- An alternating bedtime calendar and atomic paired swaps that preserve total turns. Bedtime earns points only when explicitly recorded, once per night.
- Separate secure cookie logins, account recovery, shared updates, backups, and an installable app shell.

Recording needs connectivity. Household updates appear within five seconds or on window focus. If a response is lost, **Retry change** checks the same request without double counting; unresolved requests survive a reload in that browser tab. The PWA caches its static shell, rather than private household data. HTTPS on Android supports **Add to home screen → Install**.

## Development

```sh
nix develop
npm ci
npm run dev
# In another Nix shell:
cargo run --manifest-path backend/Cargo.toml
```

Open http://localhost:3101. Vite proxies `/api` to port 3002. The default data directory is `data/`, excluded from Git.

```sh
nix develop --command cargo fmt --manifest-path backend/Cargo.toml -- --check
nix develop --command cargo clippy --manifest-path backend/Cargo.toml -- -D warnings
nix develop --command cargo test --manifest-path backend/Cargo.toml
nix develop --command cargo build --manifest-path backend/Cargo.toml
nix develop --command npm run typecheck
nix develop --command npm run format:check
nix develop --command npm run build
nix develop --command npm run test:ui
nix build .#container
```

Browser checks start an isolated production server and temporary database, create two disposable accounts, check shared chores and bedtime operations, and capture screenshots into `test-results/`. Set `CHORES_TEST_BINARY` to test a different compiled backend. `PLAYWRIGHT_CHROMIUM_EXECUTABLE` is supplied by the Nix environment.

## Deployment

The Nix container serves frontend and API on port 8080 as UID/GID 10001. Persist `/data`, and set `CHORES_ORIGIN` to the exact HTTPS address. Keep one replica. For NFS, use `CHORES_SQLITE_JOURNAL_MODE=DELETE`; local/block storage defaults to WAL.

[Deployment and recovery](docs/DEPLOYMENT.md) documents homelab manifests, publishing, backups and upgrades. [The plan](docs/PLAN.md) records the user requirements and completion checks.

DM Sans and Fraunces are locally bundled from Google Fonts under their SIL Open Font Licenses in `public/fonts/`. Icons are Lucide (ISC license). The house/check app icon is original SVG artwork in `public/icon.svg`; `scripts/make-icons.mjs` renders the install icons.
