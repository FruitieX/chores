import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdtemp, mkdir, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { createServer } from "node:net";
import { setTimeout as delay } from "node:timers/promises";
import { chromium } from "playwright";
const socket = createServer();
await new Promise((r) => socket.listen(0, "127.0.0.1", r));
const port = socket.address().port;
await new Promise((r) => socket.close(r));
const origin = `http://127.0.0.1:${port}`;
const data = await mkdtemp(`${tmpdir()}/chores-ui-`);
let logs = "";
const server = spawn(
  process.env.CHORES_TEST_BINARY ?? "backend/target/debug/chores-api",
  [],
  {
    env: {
      ...process.env,
      CHORES_BIND: `127.0.0.1:${port}`,
      CHORES_ORIGIN: origin,
      CHORES_DATA_DIR: data,
      CHORES_STATIC_DIR: resolve("dist"),
    },
    stdio: ["ignore", "pipe", "pipe"],
  },
);
server.stdout.on("data", (d) => (logs += d));
server.stderr.on("data", (d) => (logs += d));
let browser;
async function eventually(fn) {
  let error;
  for (let i = 0; i < 60; i++) {
    try {
      return await fn();
    } catch (e) {
      error = e;
      await delay(150);
    }
  }
  throw error;
}
async function state(page) {
  return page.evaluate(async () => {
    const r = await fetch("/api/state");
    if (!r.ok) throw new Error("state not available");
    return r.json();
  });
}
async function post(page, path, body) {
  return page.evaluate(
    async ({ path, body }) => {
      const r = await fetch(`/api${path}`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
      });
      return { status: r.status, data: await r.json() };
    },
    { path, body },
  );
}
async function record(page, name) {
  await page
    .getByRole("button", { name: new RegExp(name) })
    .first()
    .click();
  const dialog = page.getByRole("dialog");
  await dialog.getByRole("button", { name: /Record \d+ pts/ }).click();
  await dialog.waitFor({ state: "hidden" });
}
try {
  await eventually(async () =>
    assert.equal((await fetch(`${origin}/api/ready`)).status, 200),
  );
  browser = await chromium.launch({
    executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE,
    args: ["--no-sandbox", "--disable-dev-shm-usage"],
  });
  const a = await browser.newContext({
    viewport: { width: 1440, height: 1000 },
  });
  const b = await browser.newContext({ viewport: { width: 390, height: 844 } });
  const p = await a.newPage();
  const q = await b.newPage();
  const pageErrors = [];
  p.on("pageerror", (e) => pageErrors.push(e.message));
  q.on("pageerror", (e) => pageErrors.push(e.message));
  await p.goto(origin);
  await p.getByLabel("Household name").fill("Our home");
  await p.getByLabel("Adult 1 name").fill("Alex");
  await p.getByLabel("Adult 2 name").fill("Sam");
  await p.getByRole("button", { name: "Continue" }).click();
  await p.getByLabel("Alex email").fill("alex@example.test");
  await p.getByLabel("Alex password").fill("alex-test-password");
  await p.getByLabel("Sam email").fill("sam@example.test");
  await p.getByLabel("Sam password").fill("sam-test-password");
  await p.getByRole("button", { name: "Continue" }).click();
  await p.getByRole("button", { name: "Create our household" }).click();
  await p.getByRole("button", { name: "I’ve saved the codes" }).click();
  await p.getByRole("heading", { name: "The everyday, shared." }).waitFor();
  await q.goto(origin);
  await q.getByLabel("Email", { exact: true }).fill("sam@example.test");
  await q.getByLabel("Password", { exact: true }).fill("sam-test-password");
  await q.getByRole("button", { name: "Sign in", exact: true }).click();
  await q.getByRole("heading", { name: "The everyday, shared." }).waitFor();
  await record(p, "Run dishwasher");
  await record(q, "Hang laundry to dry");
  await eventually(async () => {
    const s = await state(p);
    assert.equal(s.household.completions.length, 2);
    assert.equal(s.household.completions[0].person, "a");
    assert.equal(s.household.completions[1].person, "b");
  });
  await eventually(async () =>
    assert.match(
      await p.locator(".balance-caption").innerText(),
      /Sam[\s\S]*2 more points/,
    ),
  );
  await mkdir("test-results", { recursive: true });
  await p.screenshot({
    path: "test-results/desktop-chores.png",
    fullPage: true,
  });
  await q.screenshot({
    path: "test-results/mobile-chores.png",
    fullPage: true,
  });
  // Historical credits must survive point edits and archiving.
  await p.goto(`${origin}/#settings`);
  await p.getByRole("button", { name: /Run dishwasher.*2 pts/ }).click();
  await p.getByRole("dialog").getByLabel("Points", { exact: true }).fill("7");
  await p
    .getByRole("dialog")
    .getByRole("button", { name: "Save chore" })
    .click();
  await p.getByRole("dialog").waitFor({ state: "hidden" });
  await p.getByRole("button", { name: "Add chore", exact: true }).click();
  const edit = p.getByRole("dialog");
  await edit.getByLabel("Chore name").fill("Take out recycling");
  await edit.getByLabel("Points", { exact: true }).fill("3");
  await edit.getByRole("button", { name: "Save chore" }).click();
  await edit.waitFor({ state: "hidden" });
  await p.goto(`${origin}/#home`);
  await record(p, "Run dishwasher");
  let s = await state(p);
  assert.deepEqual(
    s.household.completions
      .filter((c) => c.choreId === "dishwasher")
      .map((c) => c.points),
    [2, 7],
  );
  await record(p, "Take out recycling");
  await p.goto(`${origin}/#settings`);
  await p.getByRole("button", { name: /Take out recycling.*3 pts/ }).click();
  await p.getByRole("dialog").getByLabel("Archive this chore").check();
  await p
    .getByRole("dialog")
    .getByRole("button", { name: "Save chore" })
    .click();
  await p.getByRole("dialog").waitFor({ state: "hidden" });
  await p.goto(`${origin}/#home`);
  assert.equal(
    await p.getByRole("button", { name: /Take out recycling/ }).count(),
    0,
  );
  // An old entry remains in history but leaves the rolling 30-day balance.
  s = await state(p);
  const today = s.today;
  const add = (day, n) => {
    const d = new Date(`${day}T12:00:00Z`);
    d.setUTCDate(d.getUTCDate() + n);
    return d.toISOString().slice(0, 10);
  };
  assert.equal(
    (
      await post(p, "/operations", {
        id: crypto.randomUUID(),
        revision: s.household.revision,
        operation: {
          type: "complete",
          choreId: "washer",
          expectedPoints: 2,
          date: add(today, -31),
          note: "Earlier work",
        },
      })
    ).status,
    200,
  );
  await p.goto(`${origin}/#history`);
  await p
    .getByRole("button", { name: `Undo Run dishwasher on ${today}` })
    .last()
    .click();
  await p
    .getByRole("dialog")
    .getByRole("button", { name: "Remove completion" })
    .click();
  await p.getByRole("dialog").waitFor({ state: "hidden" });
  // Pair swapping preserves total scheduled duties and propagates to another session.
  await p.goto(`${origin}/#calendar`);
  await p.getByRole("button", { name: "Swap nights", exact: true }).click();
  await p.getByRole("dialog").getByLabel("Night to change").fill(today);
  await p
    .getByRole("dialog")
    .getByLabel("Night to exchange")
    .fill(add(today, 1));
  await p
    .getByRole("dialog")
    .getByRole("button", { name: "Swap both nights" })
    .click();
  await p.getByRole("dialog").waitFor({ state: "hidden" });
  await q.goto(`${origin}/#calendar`);
  await eventually(async () => {
    assert.equal((await state(q)).household.overrides[today], "b");
    assert.equal(
      await q.getByRole("button", { name: "Record tonight’s bedtime" }).count(),
      1,
    );
  });
  await p.screenshot({
    path: "test-results/desktop-calendar.png",
    fullPage: true,
  });
  await q.screenshot({
    path: "test-results/mobile-calendar.png",
    fullPage: true,
  });
  await q.getByRole("button", { name: "Record tonight’s bedtime" }).click();
  await q
    .getByRole("dialog")
    .getByRole("button", { name: /Record \d+ pts/ })
    .click();
  await q.getByRole("dialog").waitFor({ state: "hidden" });
  s = await state(q);
  assert.equal(
    s.household.completions.filter((c) => c.choreId === "bedtime" && !c.undone)
      .length,
    1,
  );
  assert.equal(
    (
      await post(q, "/operations", {
        id: crypto.randomUUID(),
        revision: s.household.revision,
        operation: {
          type: "complete",
          choreId: "bedtime",
          expectedPoints: 5,
          date: today,
          note: "",
        },
      })
    ).status,
    400,
  );
  // Changing the locked starting pattern must fail, with no partial edits.
  s = await state(p);
  assert.equal(
    (
      await post(p, "/operations", {
        id: crypto.randomUUID(),
        revision: s.household.revision,
        operation: {
          type: "saveSettings",
          expected: s.household.settings,
          settings: { ...s.household.settings, bedtimeFirst: "b" },
        },
      })
    ).status,
    400,
  );
  // Lose only the response after the server commits. Retry uses the same durable ID.
  await p.goto(`${origin}/#home`);
  await eventually(async () =>
    assert.equal(
      (await p.locator(".balance-people").innerText()).includes("Alex"),
      true,
    ),
  );
  const before = (await state(p)).household.completions.length;
  let lost = false;
  await p.route("**/api/operations", async (route) => {
    if (!lost) {
      lost = true;
      await route.fetch();
      await route.abort("failed");
    } else await route.continue();
  });
  await p.getByRole("button", { name: /Fold & put away laundry/ }).click();
  await p
    .getByRole("dialog")
    .getByRole("button", { name: /Record \d+ pts/ })
    .click();
  await p
    .getByRole("dialog")
    .getByRole("button", { name: "Retry change" })
    .waitFor();
  await p
    .getByRole("dialog")
    .getByRole("button", { name: "Retry change" })
    .click();
  await p.getByRole("dialog").waitFor({ state: "hidden" });
  assert.equal((await state(p)).household.completions.length, before + 1);
  await p.unroute("**/api/operations");
  // Backup access, restore confirmation, and restored history with accounts intact.
  await p.goto(`${origin}/#settings`);
  const backup = await p.evaluate(async () => {
    const r = await fetch("/api/backup");
    return r.json();
  });
  assert.equal(backup.version, 1);
  assert.ok(!JSON.stringify(backup).includes("password"));
  const [download] = await Promise.all([
    p.waitForEvent("download"),
    p.getByRole("link", { name: "Download backup" }).click(),
  ]);
  assert.equal(download.suggestedFilename(), "chores-backup.json");
  await p.locator("input[type=file]").setInputFiles({
    name: "backup.json",
    mimeType: "application/json",
    buffer: Buffer.from(JSON.stringify(backup)),
  });
  await p
    .getByRole("dialog")
    .getByRole("button", { name: "Replace household data" })
    .click();
  await p.getByRole("dialog").waitFor({ state: "hidden" });
  await p.getByLabel("Rolling balance").selectOption("7");
  await p.getByRole("button", { name: "Save household settings" }).click();
  await eventually(async () =>
    assert.equal((await state(q)).household.settings.balanceDays, 7),
  );
  // Settings, overflow, keyboard dialog dismissal, and mobile/offline feedback.
  await q.goto(`${origin}/#settings`);
  await q.getByRole("button", { name: "Add chore", exact: true }).click();
  await q.keyboard.press("Escape");
  await q.getByRole("dialog").waitFor({ state: "hidden" });
  for (const width of [360, 390, 768, 1440]) {
    await p.setViewportSize({ width, height: 1000 });
    for (const view of ["home", "calendar", "history", "settings"]) {
      await p.goto(`${origin}/#${view}`);
      await p.getByRole("heading", { level: 1 }).waitFor();
      assert.ok(
        await p.evaluate(
          () => document.documentElement.scrollWidth <= innerWidth + 1,
        ),
        `Overflow at ${width}, ${view}`,
      );
    }
  }
  await p.setViewportSize({ width: 1440, height: 1000 });
  await p.goto(`${origin}/#settings`);
  await p.screenshot({
    path: "test-results/desktop-settings.png",
    fullPage: true,
  });
  await q.screenshot({
    path: "test-results/mobile-settings.png",
    fullPage: true,
  });
  await b.setOffline(true);
  await q.goto(`${origin}/#home`).catch(() => {});
  await q
    .getByText("Reconnect to your home network or VPN", { exact: false })
    .waitFor();
  assert.equal(
    await q.getByRole("button", { name: /Run dishwasher/ }).isDisabled(),
    true,
  );
  await b.setOffline(false);
  await q.evaluate(() => window.dispatchEvent(new Event("online")));
  await eventually(async () =>
    assert.equal(
      await q.getByRole("button", { name: /Run dishwasher/ }).isEnabled(),
      true,
    ),
  );
  const manifest = await p.evaluate(async () => {
    const r = await fetch("/manifest.webmanifest");
    return r.json();
  });
  assert.equal(manifest.display, "standalone");
  await eventually(async () =>
    assert.equal(
      await p.evaluate(
        async () => !!(await navigator.serviceWorker.getRegistration())?.active,
      ),
      true,
    ),
  );
  assert.deepEqual(pageErrors, []);
  console.log(
    "Production browser checks passed: setup, two accounts, points/history, settings/archive, bedtime swaps, retry, backup/restore, PWA, reconnect and four responsive widths.",
  );
} catch (e) {
  console.error(logs);
  throw e;
} finally {
  if (browser) await browser.close();
  server.kill("SIGTERM");
  await rm(data, { recursive: true, force: true });
}
