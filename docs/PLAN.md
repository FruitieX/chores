# Chores implementation plan

## Goal

Complete a working, tested chores sharing app, publish it to GitHub, and deploy it to the homelab through a private ingress. Verify the deployed app and leave it ready for first-time household setup.

The user authorized building, publishing, and deploying the app. They explicitly requested that completing this plan be the active goal. No production accounts or household activity should be created by verification.

## Agreed product requirements

- A simple chores sharing app with its own blue and amber identity, navy navigation, bold sans-serif headings, a checklist app icon, and large mobile controls.
- Exactly two adults, with separate logins. Names, account details, bedtime starting date and first adult are entered during first-time setup.
- **No setup token.** First launch configures the household directly. Both accounts are created together, then each person signs in independently.
- Configurable chore list and configurable point values in settings.
- Initial chores: run dishwasher, run clothes washer, hang laundry to dry, fold and put away laundry, tidy up, maintain robot vacuum, change HVAC filters, and clean drain filters.
- Compare contributions using a **rolling last-30-days balance**, adjustable in settings. Offer other rolling periods and all-time totals. Preserve all completion history when a record leaves the balance window.
- Alternate bedtime duties daily. Allow exchanging two dates, one assigned to each adult, with both changes committed together. Swaps preserve total turns; monthly totals may differ for a swap across months.
- Bedtime is schedule-only. It is not a chore, has no points, and has no completion buttons. Existing bedtime completion records retain their original values in history and are excluded from the balance.
- Keep the app simple; no AI integration.
- Name: **Chores**. Repository: **FruitieX/chores**.

## Implementation decisions

- React, TypeScript and Vite frontend; Lucide icons; Radix accessible dialogs. Locally bundle DM Sans with its license file.
- Rust, Tokio and Axum backend with SQLite. One non-root process serves the API and compiled frontend in a reproducible Nix-built container.
- Exactly two stable profile identities. Each completion is attributed to the signed-in account, rather than a profile picker.
- Passwords use Argon2. Sessions use HttpOnly, SameSite cookies and Secure cookies under HTTPS. Same-origin writes are enforced. Recovery codes are displayed once at setup and rotate on recovery/password changes.
- Atomic SQLite mutations with revision checks, expected-value guards for edits and point changes, and durable request identifiers. Independent completions can share a revision without blocking each other. Retrying an uncertain request must not award duplicate points. Conflicting edits require reviewing the latest state.
- Shared devices refresh on a short poll, window focus, reconnect and successful mutations. Recording needs connectivity; show an explicit offline state. The installable PWA caches the static shell, with no offline mutation queue in this version.
- Chore completions preserve the chore name and point value at recording time. Subsequent configuration changes apply only to new records.
- Undo removes credit but preserves a marked history entry; only its recorder can undo a completion.
- Archive chores without deleting history. Bedtime turns live only in the calendar.
- Optional repeat intervals support maintenance due status, based on the latest unremoved completion. Unrecorded maintenance is marked clearly.
- Tap an upcoming calendar night, then an opposite-person night, to confirm an atomic swap; selection persists across month navigation. Either adult can undo a swap, including after its dates pass. Later overlapping swaps must be undone first; a new pattern can supersede a swap.
- Choose a date and first adult directly in the calendar to restart daily alternation from that day. Preserve every earlier assignment. Replace later starting patterns and overrides at or after the chosen date, keeping the event history. A crossing swap keeps its earlier assignment and is marked replaced. The setup anchor stays immutable.
- Backups use version 2 and replay swaps, undos and date-specific restarts. Version 1 imports migrate automatically. SQLite upgrades transactionally to schema 2, preserving accounts, sessions, operation identifiers and all historical completion snapshots.
- Downloadable household JSON backups exclude passwords. Restore is confirmed and revision guarded, and validates paired swap history. A database backup CLI supports complete operational backups.
- Persistent NFS volume, one replica with Recreate, SQLite DELETE journal mode and synchronous EXTRA. A private Gateway API HTTPS route.

## Work and completion checklist

### Product and implementation

- [x] Establish the design and architecture, and inspect homelab conventions.
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
- [x] Create the FruitieX/chores repository, commit with Conventional Commits and push tested changes.
- [x] Add CI validation and GHCR image publication.
- [x] Add Chores homelab Flux manifests with private ingress, persistent NFS storage and pinned image.
- [x] Commit/push only task-related homelab changes; preserve unrelated work.
- [x] Verify Flux health, ready replica, persistent volume, private HTTPS and lack of public ingress exposure.
- [x] Confirm the live app is uninitialized and ready for the user's first-time setup.
- [x] Report live app and repo links and any practical limitations; mark the active goal complete only after all required work is done.

## Completion record — 2026-10-01 (original release)

Implementation, verification, source publication and homelab deployment are complete. Local and CI container tests verify read-only/non-root runtime, idempotent writes, persistence across restart, private online backups and full database restoration.

No AI integration, offline completion queue or external account recovery service is included. Recording requires a connection to the server. Names, accounts and the bedtime starting pattern are entered in first launch without a setup token. Household days use Europe/Helsinki time. CI publishes images; deployment upgrades are applied through pinned homelab manifests.

## Bedtime calendar update — 2026-10-02

The user's follow-up supersedes the original release's bedtime logging decisions.

- [x] Remove bedtime from the chore catalog, point balance and all recording actions.
- [x] Preserve legacy bedtime records and their original point values in history.
- [x] Select two nights directly in the calendar, with a confirmation preview and cross-month selection.
- [x] Let either adult undo swaps; preserve atomic paired changes and show dependency/replacement reasons.
- [x] Add “New starting date” and selected-day “Start pattern here”, preserving all prior dates.
- [x] Migrate existing households, keep separate accounts and accept older household backups.
- [x] Complete Rust, frontend, browser, screenshot and container checks.
  - All 17 Rust tests, formatting, Clippy, frontend type checking/build and formatting pass. Two-account browser checks cover tap-to-swap, cross-month selection, shared undo, dated restarts, legacy backup migration, historical values and balance exclusion, PWA, reconnect and four responsive widths. Mobile and desktop screenshots were inspected. Nix container checks pass for persistence, idempotency, private backup/full restoration and non-root/read-only operation.
- [x] Commit/push the update to the source repository and pass CI.
- [x] Take a database backup, deploy a pinned image and verify the private ingress.

## Distinct visual identity — 2026-10-02

The user requested a distinct look and feel so Chores is recognizable among their other apps.

- [x] Give Chores blue actions, amber/blue adult colors, white cards and navy navigation.
- [x] Use bold sans-serif headings and a recognizable icon showing two checklist cards, including browser and installed app icons.
- [x] Make completed-chore recording clearer with checkmark affordances and responsive card feedback; keep the existing flows and data rules.
- [x] Verify Rust/frontend checks, production browser flows, mobile/desktop screenshots and the production container.
  - All 17 Rust tests, formatting and Clippy pass; frontend type checking, formatting and production build pass. Two-account browser flows pass at 360, 390, 768 and 1440 px. Home, calendar, settings and dialog screenshots were inspected. Container persistence, idempotency and private backup/restore checks pass.
- [x] Push the source, pass CI, deploy a pinned image and verify private HTTPS.
