# Deployment and recovery

Chores uses a private GitHub repository and GHCR image, a homelab Flux deployment, an NFS-backed SQLite database, and the private Envoy HTTPS gateway at **https://chores.fruitiex.org**. Live manifests are in `~/homelab/apps/chores`; reusable copies are under `deploy/kubernetes/`.

## First launch

Open the private HTTPS address while on LAN/VPN. Enter your household name, the two names, a separate email and password for each account, and a known bedtime date and assignee. First launch needs no setup token. Setup creates both accounts atomically and becomes unavailable afterward. Save both recovery codes separately in your password managers. The first adult is signed in automatically; the other can sign in on their own device immediately.

Emails are login identifiers; Chores does not send email. Forgotten passwords use the account's saved recovery code. Password recovery and changes replace the recovery code and sign out that person's existing sessions. Each account can record and undo its own chores; either adult can change shared chore/settings values and exchange bedtime nights. The first account can restore the household backup.

## Build and publication

```sh
nix build .#container
docker load -i result
```

CI on main validates Rust, frontend, the real browser flows, and the Nix container, then publishes `ghcr.io/fruitiex/chores:main` and a source revision tag. The repository and package are private. Kubernetes uses the existing `ghcr-credentials` pull secret. Deploy an immutable image digest in the homelab manifest after the build passes, then commit/push the homelab change for Flux. Publication does not itself update the cluster.

## Kubernetes

Use one replica with Recreate, UID/GID 10001, a read-only root filesystem and writable `/tmp`. Mount persistent data at `/data`. The supplied route attaches only to `envoy-gateway-system/private` on its HTTPS listener; no public route exists. `CHORES_ORIGIN=https://chores.fruitiex.org` must exactly match the address users visit. The existing wildcard certificate and private DNS cover the host.

The NFS claim `chores-data-nfs` uses `nfs-csi`. It has Flux pruning disabled to protect household data if manifests are removed. NFS requires `CHORES_SQLITE_JOURNAL_MODE=DELETE` and working filesystem locks and durable writes. SQLite [documents that WAL does not work on network filesystems](https://www.sqlite.org/wal.html); Chores uses synchronous EXTRA with rollback journaling. Do not run multiple app replicas on the same SQLite file.

Health: `/api/health` reports process liveness; `/api/ready` checks the database. `/api/auth/status` exposes whether first launch has been completed, without revealing household details. Authenticated household data is not cached by the service worker.

```sh
KUBECONFIG=~/homelab/kubeconfig nix develop --command kubectl get kustomization chores -n flux-system
KUBECONFIG=~/homelab/kubeconfig nix develop --command kubectl rollout status deployment/chores
```

## Backups

Settings downloads household JSON containing chores, historical credits and bedtime swaps, excluding account credentials. Restore is available to the first adult, requires confirmation and checks the current revision. The import validates the full paired swap history and the resulting assignments. Download a current backup before replacing shared data.

For a complete backup including accounts and sessions, use the SQLite online backup command. Choose a new filename; the command refuses to overwrite an existing file.

```sh
kubectl exec deploy/chores -- chores-api --backup /data/backup-2026-10-01.sqlite3
kubectl cp default/$(kubectl get pod -l app=chores -o jsonpath='{.items[0].metadata.name}'):/data/backup-2026-10-01.sqlite3 ./chores-backup.sqlite3
```

Keep backups outside the PVC and verify restoration periodically. A live SQLite main-file copy alone is not a consistent backup, especially under WAL.

## Recovery and upgrades

Before upgrades, take a database backup and pin the new image digest. Flux replaces the single pod; application schema checks refuse newer database formats on older binaries. Close and reopen installed app windows after upgrading so the new service worker can activate.

For full restoration, suspend the Chores Flux Kustomization, scale its deployment to zero, restore `chores.sqlite3` from a consistent backup with owner UID/GID 10001, and remove obsolete WAL/SHM files while the app is stopped. Restart the deployment, check readiness and both logins, then resume Flux. Never replace the database under a running server.

A user who loses both password and recovery code can restore a known full database backup. There is no remote account recovery service. Do not delete the live volume to reset a household unless loss of all its data is explicitly intended.
