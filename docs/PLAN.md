# Chores implementation and deployment plan

## Goal

Complete a working, tested chores sharing app in `/home/rasse/chores`, push it to a private GitHub repository, and deploy it to the homelab at **https://chores.fruitiex.org** through the private ingress. Verify the deployed app and leave it ready for first-time household setup.

The user authorized building, publishing to a private repository, and deploying the app. They explicitly requested that completing this plan be the active goal. No production accounts or household activity should be created by verification.

## Agreed product requirements

- A simpler companion to `~/forkbasket`, using a similar warm cream and forest green design, friendly serif headings, and large mobile controls.
- Exactly two adults, with separate logins. Names, account details, bedtime starting date and first adult are entered during first-time setup.
- **No setup token.** First launch configures the household directly. Both accounts are created together, then each person signs in independently.
- Configurable chore list and configurable point values in settings.
- Initial chores: run dishwasher, run clothes washer, hang laundry to dry, fold and put away laundry, tidy up, put kids to sleep, maintain robot vacuum, change HVAC filters, and clean drain filters.
- Compare contributions using a **rolling last-30-days balance**, adjustable in settings. Offer other rolling periods and all-time totals. Preserve all completion history when a record leaves the balance window.
- Alternate bedtime duties daily. Allow exchanging two dates, one assigned to each adult, with both changes committed together. Swaps preserve total turns; monthly totals may differ for a swap across months.
- Explicitly record completed bedtime duties for points and actual-night history. Do not automatically award points from scheduled duties.
- Keep the app simple; no AI integration.
- Name: **Chores**. Private address: **https://chores.fruitiex.org**. Private repository: **FruitieX/chores**.

## Implementation decisions

- React, TypeScript and Vite frontend; Lucide icons; Radix accessible dialogs. Reuse ForkBasket's locally bundled DM Sans and Fraunces fonts with license files.
- Rust, Tokio and Axum backend with SQLite. One non-root process serves the API and compiled frontend in a reproducible Nix-built container.
- Exactly two stable profile identities. Each completion is attributed to the signed-in account, rather than a profile picker.
- Passwords use Argon2. Sessions use HttpOnly, SameSite cookies and Secure cookies under HTTPS. Same-origin writes are enforced. Recovery codes are displayed once at setup and rotate on recovery/password changes.
- Atomic SQLite mutations with revision checks, expected-value guards for edits and point changes, and durable request identifiers. Independent completions can share a revision without blocking each other. Retrying an uncertain request must not award duplicate points. Conflicting edits require reviewing the latest state.
- Shared devices refresh on a short poll, window focus, reconnect and successful mutations. Recording needs connectivity; show an explicit offline state. The installable PWA caches the static shell, with no offline mutation queue in this version.
- Chore completions preserve the chore name and point value at recording time. Subsequent configuration changes apply only to new records.
- Undo removes credit but preserves a marked history entry; only its recorder can undo a completion.
- Archive chores without deleting history. Bedtime remains a dedicated calendar chore with configurable points.
- Optional repeat intervals support maintenance due status, based on the latest unremoved completion. Unrecorded maintenance is marked clearly.
- Bedtime starting pattern can be changed until the first bedtime completion or swap, then is preserved. Swaps are available for uncompleted nights today or later, and must exchange different assignees.
- Downloadable household JSON backups exclude passwords. Restore is confirmed and revision guarded, and validates paired swap history. A database backup CLI supports complete operational backups.
- Persistent homelab NFS volume, one replica with Recreate, SQLite DELETE journal mode and synchronous EXTRA. Existing private Envoy Gateway HTTPS route and GHCR pull credentials.

## Work and completion checklist

### Product and implementation

- [x] Inspect ForkBasket design, architecture and homelab conventions.
- [x] Collect initial product answers and record the accepted decisions.
- [x] Initialize the project, locked Nix environment, frontend and Rust backend dependencies.
- [x] Implement household setup without a setup token, separate logins and account recovery.
- [x] Implement chore logging, configurable point values, archive/restore and repeat intervals.
- [x] Implement rolling balance, historical point snapshots and guarded undo.
- [x] Implement alternating bedtime calendar, paired swaps and explicit bedtime logging.
- [x] Implement shared refresh, connection feedback and idempotent mutation retries.
- [x] Implement responsive visual design and accessible dialogs.
- [x] Implement household backup download/restore and database backup command.
- [x] Finish compiling, resolve any defects and review edge cases.
- [x] Finish installable PWA icons and static shell caching.

### Verification

- [x] Rust formatting, Clippy and meaningful domain/API tests pass.
- [x] Frontend type checking, formatting and production build pass.
- [x] Production browser checks cover setup, separate accounts, two-device updates, balance, chore settings, historical values, undo, bedtime swapping and uniqueness, backups, responsive layouts, and reconnect feedback.
- [x] Capture and inspect real mobile/desktop screenshots.
- [x] Build the reproducible container and verify runtime health and persisted data after restart.

### Publish and deployment

- [x] Write README and operational deployment/recovery instructions.
- [ ] Create private FruitieX/chores repository, commit with Conventional Commits and push tested changes.
- [ ] Add CI validation and private GHCR image publication.
- [ ] Add Chores homelab Flux manifests with private ingress, persistent NFS storage and pinned image.
- [ ] Commit/push only task-related homelab changes; preserve unrelated work, including the existing ForkBasket README edit.
- [ ] Verify Flux health, ready replica, persistent volume, private HTTPS and lack of public ingress exposure.
- [ ] Confirm the live app is uninitialized and ready for the user's first-time setup.
- [ ] Report live app and private repo links and any practical limitations; mark the active goal complete only after all required work is done.

## Current progress

The app implementation passes Rust tests, Clippy, frontend type checking/build, and production browser checks with two separate accounts at 360, 390, 768 and 1440 px. Browser flows cover first-launch setup, independent contributions, historical point values, archiving, undo, paired bedtime swaps and unique bedtime completion, backup/restore, configurable balance, PWA assets, and connection recovery. A lost-response simulation confirms retrying a committed completion awards points only once. Real mobile/desktop chore and calendar screenshots have been inspected. The Nix container builds and passes non-root/read-only runtime, restart/session/data persistence, idempotency, private online backup and complete backup restoration checks. Expected-value guards additionally protect editors that remain open while the partner updates configuration.

Private FruitieX/chores has been created; source push and CI/private GHCR publication are next. Deployment templates use only the private gateway and allow existing ExternalDNS to point the hostname at its LAN address. Final source/container checks will be repeated for the small focus-restoration and regression-test additions before publishing.
