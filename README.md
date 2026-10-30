# Chores

A simple app for sharing the work at home. Two adults record their own chores, compare a rolling points balance, and alternate bedtime duties with fair two-night swaps.

It is meant to run on a private network behind an HTTPS reverse proxy. First launch walks you through household names, both accounts, and the bedtime starting night; no setup token is needed. Save each account's recovery code in its password manager.

- Configurable chores, points, categories, icons and optional maintenance intervals.
- A rolling last-30-days balance, adjustable to other periods or all time.
- Historical point values, completion notes and dates, and undo with a preserved history entry.
- An alternating bedtime calendar and atomic paired swaps that preserve total turns. Tap two calendar nights to swap them, or undo a swap. Start a new alternating pattern from a chosen date while preserving earlier nights. Bedtime has no chore points or recording step.
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

The Nix container serves frontend and API on port 8080 as UID/GID 10001. Persist `/data`, and set `CHORES_ORIGIN` to the exact HTTPS address. Keep one replica. For NFS, use `CHORES_SQLITE_JOURNAL_MODE=DELETE`; local/block storage defaults to WAL. If a gateway injects a feedback widget, set `CHORES_FEEDBACK_HUB` to its origin (e.g. `https://feedback.example.com`) so the Content-Security-Policy allows it.

### Passkey sign-in (OpenID Connect)

Optionally, Chores can sign people in through an OpenID Connect provider such as Pocket ID. Without these variables, only password sign-in exists, as before.

| Variable                     | Meaning                                                                                                                                                                                               |
| ---------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `CHORES_OIDC_ISSUER`         | Provider issuer URL, e.g. `https://id.fruitiex.org`                                                                                                                                                   |
| `CHORES_OIDC_CLIENT_ID`      | Client id registered at the provider (`chores`)                                                                                                                                                       |
| `CHORES_OIDC_CLIENT_SECRET`  | Client secret (keep it in a secret)                                                                                                                                                                   |
| `CHORES_OIDC_ALLOWED_GROUPS` | Comma-separated groups; the `groups` claim must contain one. Empty allows anyone the provider admits.                                                                                                 |
| `CHORES_OIDC_USER_MAP`       | Comma-separated `provideruser:account` pairs linking provider usernames (`preferred_username`, case-insensitive) to the local accounts, given as `a`/`b` or the account email, e.g. `rasse:a,julle:b` |

The first three must be set together. Register `${CHORES_ORIGIN}/auth/callback` as the redirect URL. With them set, the login screen leads with **Sign in with passkey**; **Use password instead** keeps password sign-in, recovery and the first-launch setup available.

On a person's first provider sign-in, the user map links their provider identity (issuer and `sub`) to the account permanently; later sign-ins use that link even if the username changes. An account links to one identity only, and provider users without a mapping are refused, so no accounts are created. Provider sessions last 365 days and renew themselves while the app is used; password sessions keep their fixed 90 days.

[Deployment and recovery](docs/DEPLOYMENT.md) documents Kubernetes manifests, publishing, backups and upgrades. [The plan](docs/PLAN.md) records the user requirements and completion checks.

DM Sans is locally bundled from Google Fonts under its SIL Open Font License in `public/fonts/`. Icons are Lucide (ISC license). The app icon depicts two checklist cards, drawn as original SVG artwork in `public/icon.svg`; `scripts/make-icons.mjs` renders the install icons.
