# Deployment and recovery

Chores ships as a GHCR container image and is deployed with Flux from the [homelab](https://github.com/FruitieX/homelab) repository (`apps/chores`), with an NFS-backed SQLite database behind a private HTTPS gateway. Reusable manifests are under `deploy/kubernetes/`.

## First launch

Open the app's HTTPS address. Enter your household name, the two names, a separate email and password for each account, and a known bedtime date and assignee. First launch needs no setup token. Setup creates both accounts atomically and becomes unavailable afterward. Save both recovery codes separately in your password managers. The first adult is signed in automatically; the other can sign in on their own device immediately.

Emails are login identifiers; Chores does not send email. Forgotten passwords use the account's saved recovery code. Password recovery and changes replace the recovery code and sign out that person's existing sessions. Each account can record and undo its own chores; either adult can change shared chore/settings values and exchange or undo bedtime swaps and restart alternation from a chosen day. The first account can restore the household backup.

## Build and publication

```sh
nix build .#container
docker load -i result
```

CI on main validates Rust, frontend, the real browser flows, and the Nix container, then publishes `ghcr.io/fruitiex/chores:main` and a source revision tag. If the package is private, give Kubernetes a registry pull secret. Deploy an immutable image digest in your Flux manifests after the build passes.

## Kubernetes

Use one replica with Recreate, UID/GID 10001, a read-only root filesystem and writable `/tmp`. Mount persistent data at `/data`. The supplied route attaches only to a private gateway's HTTPS listener; no public route exists. `CHORES_ORIGIN` (e.g. `https://chores.example.com`) must exactly match the address users visit. Provide DNS and a TLS certificate for the host.

The example NFS claim has Flux pruning disabled to protect household data if manifests are removed. NFS requires `CHORES_SQLITE_JOURNAL_MODE=DELETE` and working filesystem locks and durable writes. SQLite [documents that WAL does not work on network filesystems](https://www.sqlite.org/wal.html); Chores uses synchronous EXTRA with rollback journaling. Do not run multiple app replicas on the same SQLite file.

Health: `/api/health` reports process liveness; `/api/ready` checks the database. `/api/auth/status` exposes whether first launch has been completed, without revealing household details. Authenticated household data is not cached by the service worker.

```sh
nix develop --command kubectl get kustomization chores -n flux-system
nix develop --command kubectl rollout status deployment/chores
```

## Backups

Settings downloads household JSON containing chores, historical credits and bedtime swaps, excluding account credentials. Restore is available to the first adult, requires confirmation and checks the current revision. Version 2 imports replay paired swaps, undo actions and dated pattern changes to validate the resulting assignments; version 1 imports migrate automatically. Legacy bedtime completion values remain in history and are excluded from chore totals. Download a current backup before replacing shared data.

For a complete backup including accounts and sessions, use the SQLite online backup command. Choose a new filename; the command refuses to overwrite an existing file.

```sh
kubectl exec deploy/chores -- chores-api --backup /data/backup-2026-10-01.sqlite3
kubectl cp default/$(kubectl get pod -l app=chores -o jsonpath='{.items[0].metadata.name}'):/data/backup-2026-10-01.sqlite3 ./chores-backup.sqlite3
```

Keep backups outside the PVC and verify restoration periodically. A live SQLite main-file copy alone is not a consistent backup, especially under WAL.

## Recovery and upgrades

Before upgrades, take a database backup and pin the new image digest. The bedtime calendar update migrates SQLite from schema 1 to schema 2 transactionally; rolling back to the original release requires restoring a schema 1 database backup. Passkey sign-in adds schema 3 (linked provider identities and renewing sessions); rolling back past it likewise requires a schema 2 backup. Flux replaces the single pod; application schema checks refuse newer database formats on older binaries. Close and reopen installed app windows after upgrading so the new service worker can activate.

For full restoration, suspend the Chores Flux Kustomization, scale its deployment to zero, restore `chores.sqlite3` from a consistent backup with owner UID/GID 10001, and remove obsolete WAL/SHM files while the app is stopped. Restart the deployment, check readiness and both logins, then resume Flux. Never replace the database under a running server.

A user who loses both password and recovery code can restore a known full database backup. There is no remote account recovery service. Do not delete the live volume to reset a household unless loss of all its data is explicitly intended.
