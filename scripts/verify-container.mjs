import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { execFileSync } from "node:child_process";
import { setTimeout as delay } from "node:timers/promises";
const image = process.env.CHORES_TEST_IMAGE ?? "ghcr.io/fruitiex/chores:main";
const suffix = randomUUID().slice(0, 8);
const volume = `chores-check-${suffix}`;
const names = [];
function docker(...args) {
  return execFileSync("docker", args, {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  }).trim();
}
function start(name, data = "/data") {
  names.push(name);
  docker(
    "run",
    "-d",
    "--name",
    name,
    "--read-only",
    "--cap-drop=ALL",
    "--security-opt=no-new-privileges",
    "--tmpfs",
    "/tmp:rw,noexec,nosuid,size=64m",
    "-p",
    "127.0.0.1::8080",
    "-v",
    `${volume}:/data`,
    "-e",
    "CHORES_ORIGIN=http://localhost:8080",
    "-e",
    "CHORES_SQLITE_JOURNAL_MODE=DELETE",
    "-e",
    `CHORES_DATA_DIR=${data}`,
    image,
  );
  return `http://127.0.0.1:${JSON.parse(docker("inspect", name))[0].NetworkSettings.Ports["8080/tcp"][0].HostPort}`;
}
async function ready(origin) {
  for (let i = 0; i < 100; i++) {
    try {
      if ((await fetch(`${origin}/api/ready`)).ok) return;
    } catch {}
    await delay(200);
  }
  throw new Error("Container did not become ready");
}
async function api(origin, path, body, cookie) {
  const r = await fetch(`${origin}/api${path}`, {
    method: body ? "POST" : "GET",
    headers: {
      Origin: "http://localhost:8080",
      "Content-Type": "application/json",
      ...(cookie ? { Cookie: cookie } : {}),
    },
    body: body ? JSON.stringify(body) : undefined,
  });
  assert.ok(r.ok, `${path} failed: ${r.status} ${await r.clone().text()}`);
  return {
    data: await r.json(),
    cookie: r.headers.get("set-cookie")?.split(";")[0],
  };
}
try {
  docker("volume", "create", volume);
  const name = `chores-check-${suffix}`;
  let origin = start(name);
  await ready(origin);
  assert.equal(docker("exec", name, "id", "-u"), "10001");
  for (const path of [
    "/",
    "/manifest.webmanifest",
    "/sw.js",
    "/icon-192.png",
    "/icon-512.png",
  ])
    assert.equal((await fetch(origin + path)).status, 200, path);
  assert.equal((await fetch(`${origin}/api/state`)).status, 401);
  assert.equal((await api(origin, "/auth/status")).data.initialized, false);
  const { cookie } = await api(origin, "/auth/setup", {
    settings: {
      householdName: "Container test",
      names: ["Alex", "Sam"],
      balanceDays: 30,
      bedtimeStart: "2026-10-01",
      bedtimeFirst: "a",
    },
    emails: ["alex@example.test", "sam@example.test"],
    passwords: ["container-alex-pass", "container-sam-pass"],
  });
  const s = (await api(origin, "/state", undefined, cookie)).data;
  const request = {
    id: randomUUID(),
    revision: s.household.revision,
    operation: {
      type: "complete",
      choreId: "washer",
      expectedPoints: 2,
      date: s.today,
      note: "Container persistence",
    },
  };
  await api(origin, "/operations", request, cookie);
  await api(origin, "/operations", request, cookie);
  docker("exec", name, "chores-api", "--backup", "/data/test-backup.sqlite3");
  assert.equal(
    docker("exec", name, "stat", "-c", "%a", "/data/test-backup.sqlite3"),
    "600",
  );
  assert.throws(() =>
    docker("exec", name, "chores-api", "--backup", "/data/test-backup.sqlite3"),
  );
  docker("restart", name);
  origin = `http://127.0.0.1:${JSON.parse(docker("inspect", name))[0].NetworkSettings.Ports["8080/tcp"][0].HostPort}`;
  await ready(origin);
  assert.equal(
    (await api(origin, "/state", undefined, cookie)).data.household.completions
      .length,
    1,
  );
  docker("exec", name, "mkdir", "/data/restored");
  docker(
    "exec",
    name,
    "cp",
    "/data/test-backup.sqlite3",
    "/data/restored/chores.sqlite3",
  );
  const restored = start(`${name}-restored`, "/data/restored");
  await ready(restored);
  assert.equal(
    (await api(restored, "/state", undefined, cookie)).data.household
      .completions.length,
    1,
  );
  console.log(
    "Container checks passed: UID 10001, read-only root, DELETE journaling, static/PWA assets, idempotency, persistence, private backup and full restore.",
  );
} finally {
  for (const name of names) {
    try {
      docker("rm", "-f", name);
    } catch {}
  }
  try {
    docker("volume", "rm", volume);
  } catch {}
}
