import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdtemp, mkdir, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { createServer } from "node:net";
import { createServer as createHttpServer } from "node:http";
import { setTimeout as delay } from "node:timers/promises";
import { chromium } from "playwright";
const socket = createServer();
await new Promise((r) => socket.listen(0, "127.0.0.1", r));
const port = socket.address().port;
await new Promise((r) => socket.close(r));
const origin = `http://127.0.0.1:${port}`;
const data = await mkdtemp(`${tmpdir()}/chores-ui-`);
let logs = "";
function start(extraEnv = {}) {
  const child = spawn(
    process.env.CHORES_TEST_BINARY ?? "backend/target/debug/chores-api",
    [],
    {
      env: {
        ...process.env,
        CHORES_BIND: `127.0.0.1:${port}`,
        CHORES_ORIGIN: origin,
        CHORES_DATA_DIR: data,
        CHORES_STATIC_DIR: resolve("dist"),
        ...extraEnv,
      },
      stdio: ["ignore", "pipe", "pipe"],
    },
  );
  child.stdout.on("data", (d) => (logs += d));
  child.stderr.on("data", (d) => (logs += d));
  return child;
}
let server = start();
// A minimal OpenID Connect provider standing in for Pocket ID.
let idpUser = null;
const codes = new Map();
const b64url = (s) => Buffer.from(s).toString("base64url");
const idp = createHttpServer(async (req, res) => {
  const url = new URL(req.url, idpOrigin);
  const json = (body) => {
    res.writeHead(200, { "content-type": "application/json" });
    res.end(JSON.stringify(body));
  };
  if (url.pathname === "/.well-known/openid-configuration")
    return json({
      issuer: idpOrigin,
      authorization_endpoint: `${idpOrigin}/authorize`,
      token_endpoint: `${idpOrigin}/token`,
    });
  if (url.pathname === "/authorize") {
    assert.equal(url.searchParams.get("client_id"), "chores");
    assert.equal(url.searchParams.get("code_challenge_method"), "S256");
    const code = crypto.randomUUID();
    codes.set(code, {
      user: idpUser,
      nonce: url.searchParams.get("nonce"),
      challenge: url.searchParams.get("code_challenge"),
    });
    const back = new URL(url.searchParams.get("redirect_uri"));
    back.searchParams.set("code", code);
    back.searchParams.set("state", url.searchParams.get("state"));
    res.writeHead(302, { location: back.href });
    return res.end();
  }
  if (url.pathname === "/token" && req.method === "POST") {
    let body = "";
    for await (const chunk of req) body += chunk;
    const form = new URLSearchParams(body);
    const grant = codes.get(form.get("code"));
    codes.delete(form.get("code"));
    const verifier = form.get("code_verifier") ?? "";
    const challenge = (await import("node:crypto"))
      .createHash("sha256")
      .update(verifier)
      .digest("base64url");
    if (
      !grant ||
      form.get("client_secret") !== "test-secret" ||
      challenge !== grant.challenge
    ) {
      res.writeHead(400);
      return res.end();
    }
    const claims = {
      iss: idpOrigin,
      aud: "chores",
      exp: Math.floor(Date.now() / 1000) + 60,
      nonce: grant.nonce,
      ...grant.user,
    };
    return json({
      id_token: `${b64url('{"alg":"none"}')}.${b64url(JSON.stringify(claims))}.`,
      access_token: "a",
      token_type: "Bearer",
    });
  }
  res.writeHead(404);
  res.end();
});
await new Promise((r) => idp.listen(0, "127.0.0.1", r));
const idpOrigin = `http://127.0.0.1:${idp.address().port}`;
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
async function capture(page, path) {
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.mouse.move(0, 0);
  await page.locator(".toast").waitFor({ state: "hidden" });
  await page.screenshot({ path, fullPage: true });
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
  await p.getByRole("heading", { name: "Share the everyday." }).waitFor();
  await q.goto(origin);
  await q.getByLabel("Email", { exact: true }).fill("sam@example.test");
  await q.getByLabel("Password", { exact: true }).fill("sam-test-password");
  await q.getByRole("button", { name: "Sign in", exact: true }).click();
  await q.getByRole("heading", { name: "Share the everyday." }).waitFor();
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
  await capture(p, "test-results/desktop-chores.png");
  await capture(q, "test-results/mobile-chores.png");
  // Historical credits must survive point edits and archiving.
  await p.evaluate(() => window.scrollTo(0, 500));
  await p
    .getByRole("navigation", { name: "Main navigation", exact: true })
    .getByRole("link", { name: "Settings", exact: true })
    .click();
  await eventually(async () =>
    assert.equal(await p.evaluate(() => scrollY), 0),
  );
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
  // Tap-to-swap calendar, shared undo, and date-specific starting patterns.
  const dayButton = (page, date) =>
    page.locator(`.calendar-day[data-date="${date}"]`);
  async function showDay(page, date) {
    for (let i = 0; i < 24; i++) {
      if (await dayButton(page, date).count()) return dayButton(page, date);
      const visible = await page
        .locator(".calendar-day")
        .first()
        .getAttribute("data-date");
      await page
        .getByRole("button", {
          name: date < visible ? "Previous month" : "Next month",
          exact: true,
        })
        .click();
    }
    throw new Error("Calendar date unavailable");
  }
  const pickDay = async (page, date) => (await showDay(page, date)).click();
  await p.goto(`${origin}/#calendar`);
  assert.equal(
    await p.getByRole("button", { name: /Record.*bedtime/ }).count(),
    0,
  );
  await pickDay(p, today);
  await p
    .locator(".night-selection")
    .getByText(/Now choose a Sam night/)
    .waitFor();
  await capture(p, "test-results/desktop-calendar-selection.png");
  await pickDay(p, add(today, 1));
  await p
    .getByRole("dialog")
    .getByRole("button", { name: "Swap both nights" })
    .click();
  await p.getByRole("dialog").waitFor({ state: "hidden" });
  await q.goto(`${origin}/#calendar`);
  await eventually(async () =>
    assert.equal((await state(q)).household.overrides[today], "b"),
  );
  assert.equal(
    await q.getByRole("button", { name: /Record.*bedtime/ }).count(),
    0,
  );
  await dayButton(q, today).locator("svg").waitFor();
  await q.getByRole("button", { name: /Undo swap/ }).waitFor();
  await capture(p, "test-results/desktop-calendar.png");
  await capture(q, "test-results/mobile-calendar.png");
  await q.getByRole("button", { name: /Undo swap/ }).click();
  await eventually(async () =>
    assert.deepEqual((await state(p)).household.overrides, {}),
  );
  await p.reload();
  await pickDay(p, today);
  await pickDay(p, add(today, 1));
  await p
    .getByRole("dialog")
    .getByRole("button", { name: "Swap both nights" })
    .click();
  await p.getByRole("dialog").waitFor({ state: "hidden" });
  s = await state(p);
  const beforeRestart = { ...s.household.overrides };
  await pickDay(p, add(today, 2));
  await p.getByRole("button", { name: "Start pattern here" }).click();
  assert.equal(
    await p.getByRole("dialog").getByLabel("Starting date").inputValue(),
    add(today, 2),
  );
  await p
    .getByRole("dialog")
    .getByLabel("First night belongs to")
    .selectOption("b");
  await capture(p, "test-results/desktop-new-pattern.png");
  await p
    .getByRole("dialog")
    .getByRole("button", { name: "Start new pattern" })
    .click();
  await p.getByRole("dialog").waitFor({ state: "hidden" });
  s = await state(p);
  assert.equal(s.household.overrides[today], beforeRestart[today]);
  assert.equal(
    s.household.overrides[add(today, 1)],
    beforeRestart[add(today, 1)],
  );
  assert.deepEqual(s.household.bedtimeStarts.at(-1), {
    date: add(today, 2),
    person: "b",
  });
  await q.reload();
  await (
    await showDay(q, add(today, 2))
  )
    .getByText("Sam", { exact: true })
    .waitFor();
  await q.getByRole("button", { name: "New starting date" }).click();
  await q.getByRole("dialog").getByLabel("Starting date").fill(add(today, 3));
  await capture(q, "test-results/mobile-new-pattern.png");
  await q
    .getByRole("dialog")
    .getByRole("button", { name: "Cancel", exact: true })
    .click();
  assert.equal(
    s.household.chores.some((c) => c.id === "bedtime"),
    false,
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
  // Select a pair across month navigation, then undo it from the other account.
  const monthEnd = new Date(
    Date.UTC(Number(today.slice(0, 4)), Number(today.slice(5, 7)), 0),
  )
    .toISOString()
    .slice(0, 10);
  const crossFirst =
    monthEnd > add(today, 3)
      ? monthEnd
      : new Date(
          Date.UTC(Number(today.slice(0, 4)), Number(today.slice(5, 7)) + 1, 0),
        )
          .toISOString()
          .slice(0, 10);
  const crossSecond = add(crossFirst, 1);
  await pickDay(p, crossFirst);
  await pickDay(p, crossSecond);
  await p
    .getByRole("dialog")
    .getByRole("button", { name: "Swap both nights" })
    .click();
  await p.getByRole("dialog").waitFor({ state: "hidden" });
  await q.reload();
  await q
    .getByRole("button", {
      name: `Undo swap ${crossFirst} and ${crossSecond}`,
      exact: true,
    })
    .click();
  await eventually(async () =>
    assert.equal((await state(p)).household.swaps.at(-1).undone, true),
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
  assert.equal(backup.version, 2);
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
  // Older backup import keeps historical values while retiring bedtime credits.
  const legacy = structuredClone(backup);
  legacy.version = 1;
  delete legacy.household.bedtimeEvents;
  legacy.household.overrides = {};
  legacy.household.swaps = [];
  legacy.household.chores.push({
    id: "bedtime",
    name: "Put kids to sleep",
    icon: "bedtime",
    category: "Bedtime",
    points: 900,
    intervalDays: null,
    archived: false,
  });
  legacy.household.completions.push({
    id: "legacy-bedtime",
    choreId: "bedtime",
    name: "Put kids to sleep",
    icon: "bedtime",
    person: "a",
    points: 900,
    date: today,
    note: "",
    createdAt: new Date().toISOString(),
    undone: false,
  });
  s = await state(p);
  assert.equal(
    (
      await post(p, "/restore", {
        version: 1,
        household: legacy.household,
        revision: s.household.revision,
      })
    ).status,
    200,
  );
  await p.goto(`${origin}/#home`);
  await p.reload();
  await p.locator(".balance-people").waitFor();
  const migrated = (await state(p)).household;
  assert.equal(
    migrated.chores.some((c) => c.id === "bedtime"),
    false,
  );
  assert.equal(
    migrated.completions.find((c) => c.id === "legacy-bedtime").points,
    900,
  );
  assert.equal(
    await p.getByRole("button", { name: /Record.*bedtime/ }).count(),
    0,
  );
  const credits = migrated.completions.filter(
    (c) =>
      c.choreId !== "bedtime" &&
      !c.undone &&
      c.date >= add(today, -29) &&
      c.date <= today,
  );
  for (const [index, person] of ["a", "b"].entries()) {
    const expected = credits
      .filter((c) => c.person === person)
      .reduce((sum, c) => sum + c.points, 0);
    assert.equal(
      (
        await p
          .locator(".balance-people > div")
          .nth(index)
          .locator("strong")
          .innerText()
      ).trim(),
      `${expected} pts`,
    );
  }
  await p.goto(`${origin}/#history`);
  await p.getByText(/Legacy bedtime, excluded from balance/).waitFor();
  await capture(p, "test-results/desktop-legacy-history.png");
  s = await state(p);
  assert.equal(
    (await post(p, "/restore", { ...backup, revision: s.household.revision }))
      .status,
    200,
  );
  await p.goto(`${origin}/#settings`);
  await p.reload();
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
  await capture(p, "test-results/desktop-settings.png");
  await capture(q, "test-results/mobile-settings.png");
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
  // Passkey sign-in through the provider, with password sign-in as a fallback.
  server.kill("SIGTERM");
  await new Promise((r) => server.once("exit", r));
  server = start({
    CHORES_OIDC_ISSUER: idpOrigin,
    CHORES_OIDC_CLIENT_ID: "chores",
    CHORES_OIDC_CLIENT_SECRET: "test-secret",
    CHORES_OIDC_ALLOWED_GROUPS: "family",
    CHORES_OIDC_USER_MAP: "alexuser:a,samuser:sam@example.test",
  });
  await eventually(async () =>
    assert.equal((await fetch(`${origin}/api/ready`)).status, 200),
  );
  const c = await browser.newContext({ viewport: { width: 390, height: 844 } });
  const r = await c.newPage();
  r.on("pageerror", (e) => pageErrors.push(e.message));
  await r.goto(origin);
  const passkeyLink = r.getByRole("link", { name: "Sign in with passkey" });
  await passkeyLink.waitFor();
  assert.equal(await r.getByLabel("Password", { exact: true }).count(), 0);
  await capture(r, "test-results/mobile-passkey-login.png");
  for (const [user, message] of [
    [
      { sub: "kid", preferred_username: "kid", groups: ["kids"] },
      /isn’t allowed/,
    ],
    [
      { sub: "guest", preferred_username: "guest", groups: ["family"] },
      /isn’t linked/,
    ],
  ]) {
    idpUser = user;
    await passkeyLink.click();
    await r.getByRole("alert").filter({ hasText: message }).waitFor();
    assert.equal(new URL(r.url()).search, "");
  }
  idpUser = {
    sub: "alex-sub",
    preferred_username: "AlexUser",
    groups: ["family"],
  };
  await passkeyLink.click();
  await r.getByRole("heading", { name: "Share the everyday." }).waitFor();
  assert.equal((await state(r)).person, "a");
  const session = (await c.cookies()).find((k) => k.name === "chores_session");
  assert.ok(session.expires > Date.now() / 1000 + 364 * 86400);
  const d = await browser.newContext({
    viewport: { width: 1440, height: 1000 },
  });
  const t = await d.newPage();
  t.on("pageerror", (e) => pageErrors.push(e.message));
  await t.goto(origin);
  await t.getByRole("button", { name: "Use password instead" }).click();
  await t.getByLabel("Email", { exact: true }).fill("sam@example.test");
  await t.getByLabel("Password", { exact: true }).fill("sam-test-password");
  await t.getByRole("button", { name: "Sign in", exact: true }).click();
  await t.getByRole("heading", { name: "Share the everyday." }).waitFor();
  assert.equal((await state(t)).person, "b");
  assert.deepEqual(pageErrors, []);
  console.log(
    "Production browser checks passed: setup, two accounts, points/history, settings/archive, bedtime swaps/undo/restarts, retry, backup/restore, PWA, reconnect, four responsive widths and passkey sign-in with password fallback.",
  );
} catch (e) {
  console.error(logs);
  throw e;
} finally {
  if (browser) await browser.close();
  server.kill("SIGTERM");
  idp.close();
  await rm(data, { recursive: true, force: true });
}
