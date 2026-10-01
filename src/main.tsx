import React, {
  useEffect,
  useRef,
  useState,
  createContext,
  useContext,
} from "react";
import { createRoot } from "react-dom/client";
import * as Dialog from "@radix-ui/react-dialog";
import {
  House,
  Check,
  CalendarDays,
  History,
  Settings2,
  LogOut,
  Plus,
  ChevronLeft,
  ChevronRight,
  X,
  ArrowLeftRight,
  Moon,
  WashingMachine,
  Shirt,
  Wind,
  Filter,
  Sparkles,
  Bot,
  Utensils,
  Download,
  RotateCcw,
  CheckCheck,
  WifiOff,
  CircleHelp,
} from "lucide-react";
import type {
  Chore,
  Household,
  Operation,
  Person,
  Settings,
  Snapshot,
} from "./types";
import { addDays, assignee, formatDate, personName } from "./types";
import "./styles.css";

const icons: Record<string, typeof House> = {
  dishwasher: Utensils,
  washer: WashingMachine,
  hang: Wind,
  fold: Shirt,
  tidy: Sparkles,
  bedtime: Moon,
  robot: Bot,
  filter: Filter,
  drain: Filter,
  custom: CheckCheck,
};
function ChoreIcon({ value, size = 28 }: { value: string; size?: number }) {
  const Icon = icons[value] ?? CheckCheck;
  return <Icon size={size} strokeWidth={1.6} />;
}
function Brand() {
  return (
    <span className="brand">
      <span className="brand-mark">
        <House size={24} />
      </span>
      Chores<span className="brand-dot">.</span>
    </span>
  );
}
class ApiError extends Error {
  constructor(
    message: string,
    public status: number,
  ) {
    super(message);
  }
}
async function api(path: string, body?: unknown): Promise<any> {
  const response = await fetch(`/api${path}`, {
    method: body === undefined ? "GET" : "POST",
    headers: body === undefined ? {} : { "Content-Type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  let data;
  try {
    data = await response.json();
  } catch {
    if (!response.ok)
      throw new ApiError(
        "The request was not valid. Check the entered details or backup file.",
        response.status,
      );
    throw new Error(
      "The server response could not be read. Retry to check your change.",
    );
  }
  if (!response.ok)
    throw new ApiError(
      data.message ?? "Something went wrong.",
      response.status,
    );
  return data;
}
const ErrorContext = createContext<React.ReactNode>(null);
function Modal({
  title,
  description,
  children,
  onClose,
}: {
  title: string;
  description?: string;
  children: React.ReactNode;
  onClose: () => void;
}) {
  const notice = useContext(ErrorContext);
  const previousFocus = useRef(document.activeElement as HTMLElement | null);
  useEffect(() => {
    window.addEventListener("chores:retry-success", onClose);
    return () => window.removeEventListener("chores:retry-success", onClose);
  }, [onClose]);
  return (
    <Dialog.Root
      open
      onOpenChange={(v) => {
        if (!v) onClose();
      }}
    >
      <Dialog.Portal>
        <Dialog.Overlay className="overlay" />
        <Dialog.Content
          className="modal"
          onCloseAutoFocus={(event) => {
            event.preventDefault();
            if (previousFocus.current?.isConnected)
              previousFocus.current.focus({ preventScroll: true });
          }}
        >
          <Dialog.Title>{title}</Dialog.Title>
          <Dialog.Description
            className={description ? "modal-description" : "sr-only"}
          >
            {description ?? title}
          </Dialog.Description>
          <Dialog.Close className="close" aria-label="Close dialog">
            <X size={20} />
          </Dialog.Close>
          {notice}
          {children}
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
function Field({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <label className="field">
      <span>{label}</span>
      {children}
    </label>
  );
}
function Recovery({
  data,
  h,
  onClose,
}: {
  data: any;
  h: Household;
  onClose: () => void;
}) {
  const entries: Array<{ person: Person; code: string }> = Array.isArray(data)
    ? [
        { person: "a", code: data[0] },
        { person: "b", code: data[1] },
      ]
    : [data];
  return (
    <Modal
      title="Save your recovery codes"
      description="Keep each code in that person’s password manager. It lets them reset a forgotten password. These codes are shown only once."
      onClose={onClose}
    >
      {entries.map((e) => (
        <div className="recovery" key={e.person}>
          <strong>{personName(h, e.person)}</strong>
          <code>{e.code}</code>
          <button
            className="text-button"
            onClick={() => navigator.clipboard.writeText(e.code)}
          >
            Copy code
          </button>
        </div>
      ))}
      <button className="primary full" onClick={onClose}>
        I’ve saved the codes
      </button>
    </Modal>
  );
}
function Auth({
  initialized,
  onDone,
}: {
  initialized: boolean;
  onDone: (r: any) => Promise<void>;
}) {
  const [mode, setMode] = useState(initialized ? "login" : "setup");
  const [step, setStep] = useState(0);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [email, setEmail] = useState("");
  const [pass, setPass] = useState("");
  const [code, setCode] = useState("");
  const today = new Intl.DateTimeFormat("sv-SE", {
    timeZone: "Europe/Helsinki",
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  }).format(new Date());
  const [settings, setSettings] = useState<Settings>({
    householdName: "Our home",
    names: ["", ""],
    balanceDays: 30,
    bedtimeStart: today,
    bedtimeFirst: "a",
  });
  const [emails, setEmails] = useState(["", ""]);
  const [passwords, setPasswords] = useState(["", ""]);
  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError("");
    if (mode === "setup" && step < 2) {
      setStep(step + 1);
      return;
    }
    setBusy(true);
    try {
      const result = await api(
        `/auth/${mode}`,
        mode === "setup"
          ? { settings, emails, passwords }
          : mode === "recover"
            ? { email, password: pass, code }
            : { email, password: pass },
      );
      await onDone(result);
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="auth-shell">
      <div className="auth-brand">
        <Brand />
      </div>
      <main className="auth-card">
        <span className="eyebrow">A fairer share of the everyday</span>
        <h1>
          {mode === "setup"
            ? [
                "Make yourselves at home",
                "Your two accounts",
                "A turn for each of you",
              ][step]
            : mode === "recover"
              ? "Recover your account"
              : "Welcome home"}
        </h1>
        <p className="intro">
          {mode === "setup"
            ? [
                "A few details, then you’re ready to share the load.",
                "Each person signs in to record their own chores.",
                "Choose a starting night. Bedtime alternates every day.",
              ][step]
            : mode === "recover"
              ? "Use your saved recovery code to choose a new password."
              : "Sign in to your household."}
        </p>
        {mode === "setup" && (
          <div className="setup-progress" aria-label={`Step ${step + 1} of 3`}>
            {[0, 1, 2].map((s) => (
              <span className={s <= step ? "active" : ""} key={s} />
            ))}
          </div>
        )}
        <form onSubmit={submit}>
          {mode === "setup" ? (
            <>
              {step === 0 && (
                <>
                  <Field label="Household name">
                    <input
                      required
                      maxLength={80}
                      value={settings.householdName}
                      onChange={(e) =>
                        setSettings({
                          ...settings,
                          householdName: e.target.value,
                        })
                      }
                    />
                  </Field>
                  {[0, 1].map((i) => (
                    <Field label={`Adult ${i + 1} name`} key={i}>
                      <input
                        required
                        maxLength={40}
                        autoComplete="off"
                        value={settings.names[i]}
                        onChange={(e) => {
                          const names = [...settings.names] as [string, string];
                          names[i] = e.target.value;
                          setSettings({ ...settings, names });
                        }}
                      />
                    </Field>
                  ))}
                </>
              )}
              {step === 1 && (
                <>
                  {[0, 1].map((i) => (
                    <fieldset key={i}>
                      <legend>{settings.names[i]}</legend>
                      <Field label={`${settings.names[i]} email`}>
                        <input
                          type="email"
                          required
                          autoComplete="username"
                          value={emails[i]}
                          onChange={(e) =>
                            setEmails(
                              emails.map((v, j) =>
                                j === i ? e.target.value : v,
                              ),
                            )
                          }
                        />
                      </Field>
                      <Field label={`${settings.names[i]} password`}>
                        <input
                          type="password"
                          required
                          minLength={12}
                          maxLength={256}
                          autoComplete="new-password"
                          value={passwords[i]}
                          onChange={(e) =>
                            setPasswords(
                              passwords.map((v, j) =>
                                j === i ? e.target.value : v,
                              ),
                            )
                          }
                        />
                      </Field>
                    </fieldset>
                  ))}
                  <p className="hint">
                    At least 12 characters each. You’ll receive separate
                    recovery codes.
                  </p>
                </>
              )}
              {step === 2 && (
                <>
                  <Field label="Bedtime starting date">
                    <input
                      type="date"
                      required
                      value={settings.bedtimeStart}
                      onChange={(e) =>
                        setSettings({
                          ...settings,
                          bedtimeStart: e.target.value,
                        })
                      }
                    />
                  </Field>
                  <Field label="Who takes that first night?">
                    <select
                      value={settings.bedtimeFirst}
                      onChange={(e) =>
                        setSettings({
                          ...settings,
                          bedtimeFirst: e.target.value as Person,
                        })
                      }
                    >
                      <option value="a">{settings.names[0]}</option>
                      <option value="b">{settings.names[1]}</option>
                    </select>
                  </Field>
                  <div className="info">
                    <ArrowLeftRight size={20} />
                    <p>
                      Plans change. Swap one night each to keep your total turns
                      equal.
                    </p>
                  </div>
                  <p className="hint">
                    Your points balance starts with a rolling 30-day window.
                    Chores and point values can be changed in settings.
                  </p>
                </>
              )}
            </>
          ) : (
            <>
              <Field label="Email">
                <input
                  type="email"
                  autoComplete="username"
                  required
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                />
              </Field>
              {mode === "recover" && (
                <Field label="Recovery code">
                  <input
                    required
                    value={code}
                    onChange={(e) => setCode(e.target.value)}
                  />
                </Field>
              )}
              <Field label={mode === "recover" ? "New password" : "Password"}>
                <input
                  type="password"
                  autoComplete={
                    mode === "recover" ? "new-password" : "current-password"
                  }
                  required
                  minLength={mode === "recover" ? 12 : undefined}
                  maxLength={256}
                  value={pass}
                  onChange={(e) => setPass(e.target.value)}
                />
              </Field>
            </>
          )}
          {error && (
            <p role="alert" className="error">
              {error}
            </p>
          )}
          <div className="actions">
            {mode === "setup" && step > 0 && (
              <button
                type="button"
                className="secondary"
                onClick={() => setStep(step - 1)}
                disabled={busy}
              >
                Back
              </button>
            )}
            <button className="primary" disabled={busy}>
              {busy
                ? "Please wait…"
                : mode === "setup"
                  ? step < 2
                    ? "Continue"
                    : "Create our household"
                  : mode === "recover"
                    ? "Reset password"
                    : "Sign in"}
            </button>
          </div>
        </form>
        {initialized && (
          <button
            className="text-button auth-switch"
            onClick={() => {
              setMode(mode === "login" ? "recover" : "login");
              setError("");
            }}
          >
            {mode === "login" ? "Forgot your password?" : "Back to sign in"}
          </button>
        )}
      </main>
    </div>
  );
}
const navigation = [
  { id: "home", label: "Our chores", short: "Chores", icon: House },
  {
    id: "calendar",
    label: "Bedtime calendar",
    short: "Bedtime",
    icon: CalendarDays,
  },
  { id: "history", label: "History", short: "History", icon: History },
  { id: "settings", label: "Settings", short: "Settings", icon: Settings2 },
];
function App() {
  const [initialized, setInitialized] = useState<boolean | null>(null);
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [view, setView] = useState(location.hash.slice(1) || "home");
  const [error, setError] = useState("");
  const [connected, setConnected] = useState(navigator.onLine);
  const [busy, setBusy] = useState(false);
  const [recovery, setRecovery] = useState<any>(null);
  const [toast, setToast] = useState("");
  const [pending, setPending] = useState<{
    id: string;
    revision: number;
    operation: Operation;
  } | null>(null);
  const lock = useRef(false);
  const mounted = useRef(true);
  useEffect(() => {
    if (!snapshot) return;
    try {
      const stored = JSON.parse(
        sessionStorage.getItem("chores-pending") ?? "null",
      );
      if (stored?.person === snapshot.person) setPending(stored.request);
    } catch {}
  }, [snapshot?.person]);
  const accept = (s: Snapshot) =>
    setSnapshot((old) =>
      !old ||
      old.person !== s.person ||
      s.household.revision >= old.household.revision
        ? s
        : old,
    );
  const refresh = async () => {
    try {
      const s = await api("/state");
      if (mounted.current) {
        accept(s);
        setConnected(true);
      }
    } catch (e) {
      if (mounted.current) {
        if (e instanceof ApiError && e.status === 401) {
          setSnapshot(null);
          const status = await api("/auth/status");
          setInitialized(status.initialized);
        } else setConnected(false);
      }
    }
  };
  useEffect(() => {
    mounted.current = true;
    api("/auth/status")
      .then(async (s) => {
        setInitialized(s.initialized);
        if (s.person) await refresh();
      })
      .catch(() => {
        setError(
          "Cannot reach your household. Check your connection and try again.",
        );
        setConnected(false);
      });
    const timer = setInterval(() => {
      if (document.visibilityState === "visible") refresh();
    }, 5000);
    const focus = () => refresh();
    const off = () => setConnected(false);
    window.addEventListener("focus", focus);
    window.addEventListener("online", focus);
    window.addEventListener("offline", off);
    return () => {
      mounted.current = false;
      clearInterval(timer);
      window.removeEventListener("focus", focus);
      window.removeEventListener("online", focus);
      window.removeEventListener("offline", off);
    };
  }, []);
  useEffect(() => {
    const changed = () =>
      setView(
        navigation.some((n) => n.id === location.hash.slice(1))
          ? location.hash.slice(1)
          : "home",
      );
    window.addEventListener("hashchange", changed);
    return () => window.removeEventListener("hashchange", changed);
  }, []);
  useEffect(() => {
    if (!toast) return;
    const t = setTimeout(() => setToast(""), 4000);
    return () => clearTimeout(t);
  }, [toast]);
  const send = async (request: {
    id: string;
    revision: number;
    operation: Operation;
  }): Promise<boolean> => {
    if (lock.current) return false;
    lock.current = true;
    setBusy(true);
    setError("");
    sessionStorage.setItem(
      "chores-pending",
      JSON.stringify({ person: snapshot?.person, request }),
    );
    try {
      const s = await api("/operations", request);
      accept(s);
      setConnected(true);
      setPending(null);
      sessionStorage.removeItem("chores-pending");
      setToast(
        request.operation.type === "complete"
          ? "Chore recorded"
          : request.operation.type === "swap"
            ? "Bedtime nights swapped"
            : request.operation.type === "undo"
              ? "Completion removed"
              : "Changes saved",
      );
      return true;
    } catch (e) {
      setError((e as Error).message);
      if (e instanceof ApiError) {
        setPending(null);
        sessionStorage.removeItem("chores-pending");
        await refresh();
      } else {
        setConnected(false);
        setPending(request);
        setError(
          "The connection dropped. Retry to check whether your change was saved.",
        );
      }
      return false;
    } finally {
      lock.current = false;
      setBusy(false);
    }
  };
  const mutate = (operation: Operation) =>
    snapshot && !pending
      ? send({
          id: crypto.randomUUID(),
          revision: snapshot.household.revision,
          operation,
        })
      : Promise.resolve(false);
  const authDone = async (r: any) => {
    setInitialized(true);
    setRecovery(r.recoveryCodes);
    await refresh();
  };
  if (initialized === null)
    return (
      <div className="loading">
        <Brand />
        <p>{error || "Opening your household…"}</p>
        {error && (
          <button className="primary" onClick={() => location.reload()}>
            Try again
          </button>
        )}
      </div>
    );
  if (!snapshot) return <Auth initialized={initialized} onDone={authDone} />;
  const { household: h, person, today } = snapshot;
  const name = personName(h, person);
  const disabled = busy || !connected || !!pending;
  const notice = error && (
    <div role="alert" className="error">
      {error}
      {pending ? (
        <button
          className="secondary"
          disabled={busy}
          onClick={async () => {
            if (await send(pending))
              window.dispatchEvent(new Event("chores:retry-success"));
          }}
        >
          Retry change
        </button>
      ) : (
        <button className="text-button" onClick={() => setError("")}>
          Dismiss
        </button>
      )}
    </div>
  );
  return (
    <ErrorContext.Provider value={notice}>
      <a
        className="skip-link"
        href="#main"
        onClick={(e) => {
          e.preventDefault();
          document.getElementById("main")?.focus();
        }}
      >
        Skip to content
      </a>
      <aside className="sidebar">
        <a href="#home" aria-label="Chores home">
          <Brand />
        </a>
        <div className="household">
          <span className="home-icon">
            <House size={24} />
          </span>
          <div>
            <strong>{h.settings.householdName}</strong>
            <span>{name}</span>
          </div>
        </div>
        <nav aria-label="Main navigation">
          {navigation.map((n) => (
            <a
              href={`#${n.id}`}
              className={view === n.id ? "active" : ""}
              aria-current={view === n.id ? "page" : undefined}
              key={n.id}
            >
              <n.icon size={21} />
              {n.label}
              {view === n.id && <i />}
            </a>
          ))}
        </nav>
        <div className="sidebar-bottom">
          <span className={`sync ${connected ? "" : "offline"}`}>
            <i />
            {connected ? "Household connected" : "Offline"}
          </span>
          <button
            className="text-button"
            disabled={busy || !!pending}
            onClick={async () => {
              try {
                await api("/auth/logout", {});
                setSnapshot(null);
                setRecovery(null);
              } catch (e) {
                setError((e as Error).message);
              }
            }}
          >
            <LogOut size={17} />
            Sign out
          </button>
        </div>
      </aside>
      <div className="main-shell">
        <header className="topbar">
          <a className="mobile-brand" href="#home">
            <Brand />
          </a>
          <span className="desktop-date">
            {new Intl.DateTimeFormat("en-GB", {
              weekday: "long",
              day: "numeric",
              month: "long",
              timeZone: "UTC",
            }).format(new Date(`${today}T12:00:00Z`))}
          </span>
          <div className="topbar-right">
            <span className={`sync ${connected ? "" : "offline"}`}>
              <i />
              {connected ? "Connected" : "Offline"}
            </span>
            <span className={`avatar ${person}`} title={`Signed in as ${name}`}>
              {name.slice(0, 1)}
            </span>
          </div>
        </header>
        <main id="main" tabIndex={-1}>
          <div className="global-notices">
            {!connected && (
              <div className="info">
                <WifiOff size={19} />
                <p>
                  Reconnect to your home network or VPN to record chores. Your
                  household will refresh automatically.
                </p>
              </div>
            )}
            {notice}
          </div>
          {view === "home" && (
            <Home
              h={h}
              person={person}
              today={today}
              mutate={mutate}
              disabled={disabled}
            />
          )}
          {view === "calendar" && (
            <Calendar
              h={h}
              person={person}
              today={today}
              mutate={mutate}
              disabled={disabled}
            />
          )}
          {view === "history" && (
            <HistoryView
              h={h}
              person={person}
              today={today}
              mutate={mutate}
              disabled={disabled}
            />
          )}
          {view === "settings" && (
            <SettingsView
              h={h}
              person={person}
              mutate={mutate}
              disabled={disabled}
              onRecovery={setRecovery}
              onRestore={accept}
            />
          )}
        </main>
      </div>
      <nav className="bottom-nav" aria-label="Mobile navigation">
        {navigation.map((n) => (
          <a
            href={`#${n.id}`}
            className={view === n.id ? "active" : ""}
            aria-current={view === n.id ? "page" : undefined}
            key={n.id}
          >
            <n.icon size={22} />
            <span>{n.short}</span>
          </a>
        ))}
      </nav>
      {toast && (
        <div className="toast" role="status">
          <Check size={18} />
          {toast}
        </div>
      )}
      {recovery && (
        <Recovery data={recovery} h={h} onClose={() => setRecovery(null)} />
      )}
    </ErrorContext.Provider>
  );
}
type Shared = {
  h: Household;
  person: Person;
  today: string;
  mutate: (op: Operation) => Promise<boolean>;
  disabled: boolean;
};
function Balance({ h, today }: { h: Household; today: string }) {
  const start = h.settings.balanceDays
    ? addDays(today, 1 - h.settings.balanceDays)
    : "0000";
  const entries = h.completions.filter(
    (c) => !c.undone && c.date >= start && c.date <= today,
  );
  const totals = ["a", "b"].map((p) =>
    entries.filter((c) => c.person === p).reduce((sum, c) => sum + c.points, 0),
  );
  const total = totals[0] + totals[1];
  const diff = Math.abs(totals[0] - totals[1]);
  return (
    <section className="balance-card" aria-label="Chore points balance">
      <div className="section-top">
        <span className="eyebrow">Our share</span>
        <span className="period">
          {h.settings.balanceDays
            ? `Last ${h.settings.balanceDays} days`
            : "All time"}
        </span>
      </div>
      <div className="balance-people">
        {(["a", "b"] as Person[]).map((p, i) => (
          <div key={p}>
            <span className={`avatar ${p}`}>
              {h.settings.names[i].slice(0, 1)}
            </span>
            <div>
              <span>{h.settings.names[i]}</span>
              <strong>
                {totals[i]}
                <small> pts</small>
              </strong>
            </div>
          </div>
        ))}
      </div>
      <div
        className="balance-track"
        aria-label={`${h.settings.names[0]} ${total ? Math.round((totals[0] / total) * 100) : 50} percent of points`}
      >
        <span style={{ width: `${total ? (totals[0] / total) * 100 : 50}%` }} />
      </div>
      <div className="balance-caption">
        {total === 0 ? (
          "A fresh start. Every little thing counts."
        ) : diff === 0 ? (
          "An even share. Nice teamwork."
        ) : (
          <>
            <strong>{h.settings.names[totals[0] > totals[1] ? 0 : 1]}</strong>{" "}
            has contributed {diff} more points in this window.
          </>
        )}
        <details className="balance-help">
          <summary aria-label="How balance works">
            <CircleHelp size={16} />
          </summary>
          <p>
            Points compare completed work in this rolling window. Values are set
            by you both, and each completion keeps the value it had when
            recorded. Bedtime nights are tracked separately too.
          </p>
        </details>
      </div>
    </section>
  );
}
function Home({ h, person, today, mutate, disabled }: Shared) {
  const [selected, setSelected] = useState<Chore | null>(null);
  const [category, setCategory] = useState("All chores");
  const onDuty = assignee(h, today);
  const bedtime = h.completions.find(
    (c) => c.choreId === "bedtime" && c.date === today && !c.undone,
  );
  const tasks = h.chores.filter(
    (c) =>
      !c.archived &&
      c.id !== "bedtime" &&
      (category === "All chores" || c.category === category),
  );
  return (
    <>
      <div className="page-heading">
        <div>
          <span className="eyebrow">{h.settings.householdName}</span>
          <h1>The everyday, shared.</h1>
          <p>Hi {personName(h, person)}. What have you taken care of?</p>
        </div>
        <span className="heading-decoration">
          <Sparkles size={34} />
        </span>
      </div>
      <div className="overview">
        <Balance h={h} today={today} />
        <section className="bedtime-card">
          <div className="section-top">
            <span className="eyebrow">Tonight’s bedtime</span>
            <Moon size={21} />
          </div>
          <h2>
            {bedtime
              ? "Sweet dreams."
              : onDuty
                ? `${personName(h, onDuty)}’s turn`
                : "Set your starting night"}
          </h2>
          <p>
            {bedtime
              ? `Recorded by ${personName(h, bedtime.person)} · ${bedtime.points} pts`
              : onDuty === person
                ? "A little story, a little patience. Your night."
                : onDuty
                  ? "Your night off. Tomorrow brings another turn."
                  : "The calendar begins on " +
                    formatDate(h.settings.bedtimeStart) +
                    "."}
          </p>
          <div className="bedtime-actions">
            {!bedtime && onDuty === person && (
              <button
                className="primary"
                disabled={disabled}
                onClick={() =>
                  setSelected(h.chores.find((c) => c.id === "bedtime")!)
                }
              >
                <Check size={17} />
                Record bedtime
              </button>
            )}
            <a href="#calendar" className="text-button">
              <CalendarDays size={17} />
              {bedtime ? "View calendar" : "Calendar & swaps"}
            </a>
          </div>
        </section>
      </div>
      <div className="section-heading">
        <h2>One less thing to do</h2>
        <span className="muted">Record a completed chore</span>
      </div>
      <div className="chips" aria-label="Chore categories">
        {["All chores", "Everyday", "Laundry", "Maintenance"].map((c) => (
          <button
            className={c === category ? "selected" : ""}
            key={c}
            onClick={() => setCategory(c)}
          >
            {c}
          </button>
        ))}
      </div>
      <div className="chore-grid">
        {tasks.map((c) => {
          const last = h.completions
            .filter((e) => e.choreId === c.id && !e.undone)
            .sort((a, b) => b.date.localeCompare(a.date))[0];
          const due =
            c.intervalDays &&
            (last ? addDays(last.date, c.intervalDays) : today);
          const doneToday = h.completions.filter(
            (e) => e.choreId === c.id && !e.undone && e.date === today,
          ).length;
          return (
            <button
              className={`chore-tile ${c.category.toLowerCase()}`}
              key={c.id}
              disabled={disabled}
              onClick={() => setSelected(c)}
            >
              <span className="tile-icon">
                <ChoreIcon value={c.icon} />
              </span>
              <span className="tile-points">{c.points} pts</span>
              <strong>{c.name}</strong>
              <span
                className={`tile-detail ${due && due <= today ? "due" : ""}`}
              >
                {due
                  ? due <= today
                    ? last
                      ? "Due now"
                      : "Not recorded yet"
                    : `Due ${formatDate(due, true)}`
                  : doneToday
                    ? `${doneToday} recorded today`
                    : last
                      ? `Last done ${formatDate(last.date, true)}`
                      : c.category}
              </span>
              <span className="tile-plus">
                <Plus size={17} />
              </span>
            </button>
          );
        })}
      </div>
      {!tasks.length && (
        <div className="empty">
          No chores in this category. Add one in settings.
        </div>
      )}
      <div className="quiet-note">
        <CheckCheck size={18} />
        Small things add up. Points reflect your shared idea of effort.
      </div>
      {selected && (
        <RecordChore
          chore={selected}
          today={today}
          h={h}
          person={person}
          mutate={mutate}
          disabled={disabled}
          onClose={() => setSelected(null)}
        />
      )}
    </>
  );
}
function RecordChore({
  chore,
  today,
  h,
  person,
  mutate,
  disabled,
  onClose,
}: Shared & { chore: Chore; onClose: () => void }) {
  const [date, setDate] = useState(today);
  const [note, setNote] = useState("");
  const match = chore.id !== "bedtime" || assignee(h, date) === person;
  const already =
    chore.id === "bedtime" &&
    h.completions.some(
      (c) => c.choreId === "bedtime" && c.date === date && !c.undone,
    );
  return (
    <Modal
      title={chore.name}
      description={`Record ${chore.points} points for ${personName(h, person)}.`}
      onClose={onClose}
    >
      <form
        onSubmit={async (e) => {
          e.preventDefault();
          if (
            await mutate({
              type: "complete",
              choreId: chore.id,
              expectedPoints: chore.points,
              date,
              note,
            })
          )
            onClose();
        }}
      >
        <Field label="Completed on">
          <input
            type="date"
            required
            max={today}
            min={addDays(today, -3650)}
            value={date}
            onChange={(e) => setDate(e.target.value)}
          />
        </Field>
        <Field label="Note (optional)">
          <textarea
            rows={2}
            maxLength={500}
            value={note}
            placeholder="For example, a full load of towels"
            onChange={(e) => setNote(e.target.value)}
          />
        </Field>
        {!match && (
          <p className="error">
            This is your partner’s turn. Swap nights in the calendar first.
          </p>
        )}
        {already && (
          <p className="error">Bedtime is already recorded for this date.</p>
        )}
        <div className="actions">
          <button type="button" className="secondary" onClick={onClose}>
            Cancel
          </button>
          <button className="primary" disabled={disabled || !match || already}>
            <Check size={17} />
            Record {chore.points} pts
          </button>
        </div>
      </form>
    </Modal>
  );
}
function Calendar({ h, person, today, mutate, disabled }: Shared) {
  const [month, setMonth] = useState(today.slice(0, 7));
  const [swap, setSwap] = useState<string | null>(null);
  const [selected, setSelected] = useState<Chore | null>(null);
  const first = `${month}-01`;
  const last = new Date(
    Date.UTC(Number(month.slice(0, 4)), Number(month.slice(5, 7)), 0),
  ).getUTCDate();
  const offset = (new Date(`${first}T12:00:00Z`).getUTCDay() + 6) % 7;
  const days = Array.from(
    { length: last },
    (_, i) => `${month}-${String(i + 1).padStart(2, "0")}`,
  );
  const counts = (["a", "b"] as Person[]).map(
    (p) => days.filter((d) => assignee(h, d) === p).length,
  );
  const bedtime = h.chores.find((c) => c.id === "bedtime")!;
  return (
    <>
      <div className="page-heading">
        <div>
          <span className="eyebrow">A turn for each of you</span>
          <h1>Bedtime, together.</h1>
          <p>Every other night. Swap a pair when plans change.</p>
        </div>
        <button
          className="primary"
          disabled={disabled}
          onClick={() => setSwap(today)}
        >
          <ArrowLeftRight size={18} />
          Swap nights
        </button>
      </div>
      <section className="calendar-panel">
        <div className="calendar-toolbar">
          <button
            aria-label="Previous month"
            className="icon-button"
            onClick={() => setMonth(addDays(first, -1).slice(0, 7))}
          >
            <ChevronLeft size={21} />
          </button>
          <h2>
            {new Intl.DateTimeFormat("en-GB", {
              month: "long",
              year: "numeric",
              timeZone: "UTC",
            }).format(new Date(`${first}T12:00:00Z`))}
          </h2>
          <button
            aria-label="Next month"
            className="icon-button"
            onClick={() => setMonth(addDays(first, last).slice(0, 7))}
          >
            <ChevronRight size={21} />
          </button>
          <button
            className="text-button today-button"
            onClick={() => setMonth(today.slice(0, 7))}
          >
            Today
          </button>
        </div>
        <div className="calendar-legend">
          {(["a", "b"] as Person[]).map((p, i) => (
            <span key={p}>
              <i className={p} />
              {personName(h, p)}
              <small>{counts[i]} nights</small>
            </span>
          ))}
        </div>
        <div className="calendar-grid">
          {["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"].map((d) => (
            <span className="weekday" key={d}>
              {d}
            </span>
          ))}
          {Array.from({ length: offset }, (_, i) => (
            <div key={`blank${i}`} className="calendar-blank" />
          ))}
          {days.map((d) => {
            const who = assignee(h, d);
            const done = h.completions.find(
              (c) => c.choreId === "bedtime" && c.date === d && !c.undone,
            );
            return (
              <button
                key={d}
                className={`calendar-day ${who ?? "none"} ${d === today ? "today" : ""} ${d < today ? "past" : ""}`}
                aria-label={`${formatDate(d)}: ${who ? personName(h, who) : "Before schedule"}${h.overrides[d] ? ", swapped" : ""}${done ? ", completed" : ""}`}
                disabled={!who || d < today || !!done || disabled}
                onClick={() => setSwap(d)}
              >
                <span>
                  {Number(d.slice(8))}
                  {done ? (
                    <Check size={15} />
                  ) : h.overrides[d] ? (
                    <ArrowLeftRight size={13} />
                  ) : null}
                </span>
                <strong>{who ? personName(h, who) : "—"}</strong>
                {d === today && <small>Today</small>}
              </button>
            );
          })}
        </div>
        <p className="hint calendar-hint">
          Tap an upcoming night to swap it. A pair of exchanged nights preserves
          your total turns; monthly counts can differ if a swap crosses months.
        </p>
      </section>
      <div className="calendar-foot">
        <div className="info">
          <Moon size={21} />
          <p>
            Bedtime points count when you record the completed night. The
            calendar keeps track of whose turn it is.
          </p>
        </div>
        {assignee(h, today) === person &&
          !h.completions.some(
            (c) => c.choreId === "bedtime" && c.date === today && !c.undone,
          ) && (
            <button
              className="primary"
              disabled={disabled}
              onClick={() => setSelected(bedtime)}
            >
              Record tonight’s bedtime
            </button>
          )}
      </div>
      {h.swaps.length > 0 && (
        <section className="recent-swaps">
          <h2>Recent swaps</h2>
          {h.swaps
            .slice(-5)
            .reverse()
            .map((s) => (
              <div className="swap-row" key={s.id}>
                <ArrowLeftRight size={18} />
                <p>
                  {formatDate(s.first, true)} <span>↔</span>{" "}
                  {formatDate(s.second, true)}
                  <small>Swapped by {personName(h, s.actor)}</small>
                </p>
              </div>
            ))}
        </section>
      )}
      {swap && (
        <SwapNights
          h={h}
          person={person}
          today={today}
          first={swap}
          mutate={mutate}
          disabled={disabled}
          onClose={() => setSwap(null)}
        />
      )}{" "}
      {selected && (
        <RecordChore
          chore={selected}
          h={h}
          person={person}
          today={today}
          mutate={mutate}
          disabled={disabled}
          onClose={() => setSelected(null)}
        />
      )}
    </>
  );
}
function SwapNights({
  h,
  today,
  first: initial,
  mutate,
  disabled,
  onClose,
}: Shared & { first: string; onClose: () => void }) {
  const [first, setFirst] = useState(initial);
  const [second, setSecond] = useState("");
  const a = assignee(h, first);
  const b = second ? assignee(h, second) : null;
  const completed = (d: string) =>
    h.completions.some(
      (c) => c.choreId === "bedtime" && c.date === d && !c.undone,
    );
  const valid =
    first >= today &&
    second >= today &&
    a &&
    b &&
    a !== b &&
    !completed(first) &&
    !completed(second);
  return (
    <Modal
      title="Swap two bedtime nights"
      description="Pick one night from each person. Both dates change together."
      onClose={onClose}
    >
      <form
        onSubmit={async (e) => {
          e.preventDefault();
          if (await mutate({ type: "swap", first, second })) onClose();
        }}
      >
        <Field label="Night to change">
          <input
            type="date"
            required
            min={today}
            value={first}
            onChange={(e) => setFirst(e.target.value)}
          />
        </Field>
        {a && <p className="date-owner">Currently {personName(h, a)}’s turn</p>}
        <Field label="Night to exchange">
          <input
            type="date"
            required
            min={today}
            value={second}
            onChange={(e) => setSecond(e.target.value)}
          />
        </Field>
        {second && b && (
          <p className="date-owner">Currently {personName(h, b)}’s turn</p>
        )}
        {second && !valid && (
          <p className="error">
            Choose two uncompleted nights with different people, today or later.
          </p>
        )}
        {valid && (
          <div className="swap-preview">
            <p>
              <strong>{formatDate(first, true)}</strong>
              <span>{personName(h, b)} takes over</span>
            </p>
            <ArrowLeftRight size={21} />
            <p>
              <strong>{formatDate(second, true)}</strong>
              <span>{personName(h, a)} takes over</span>
            </p>
          </div>
        )}
        <div className="actions">
          <button type="button" className="secondary" onClick={onClose}>
            Cancel
          </button>
          <button className="primary" disabled={disabled || !valid}>
            Swap both nights
          </button>
        </div>
      </form>
    </Modal>
  );
}
function HistoryView({ h, person, today, mutate, disabled }: Shared) {
  const [filter, setFilter] = useState("all");
  const [limit, setLimit] = useState(50);
  const [remove, setRemove] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const entries = h.completions
    .filter(
      (c) =>
        (filter === "all" || c.person === filter) &&
        (c.name + " " + c.note).toLowerCase().includes(query.toLowerCase()),
    )
    .sort(
      (a, b) =>
        b.date.localeCompare(a.date) || b.createdAt.localeCompare(a.createdAt),
    );
  const start = h.settings.balanceDays
    ? addDays(today, 1 - h.settings.balanceDays)
    : "0000";
  const nights = (["a", "b"] as Person[]).map(
    (p) =>
      h.completions.filter(
        (c) =>
          c.choreId === "bedtime" &&
          c.person === p &&
          !c.undone &&
          c.date >= start &&
          c.date <= today,
      ).length,
  );
  return (
    <>
      <div className="page-heading">
        <div>
          <span className="eyebrow">Every contribution counts</span>
          <h1>Our little victories.</h1>
          <p>A record of the work you’ve shared.</p>
        </div>
      </div>
      <div className="history-summary">
        <Moon size={24} />
        <div>
          <strong>Completed bedtime nights</strong>
          <p>
            {h.settings.names[0]}: {nights[0]} <span>·</span>{" "}
            {h.settings.names[1]}: {nights[1]}{" "}
            <small>
              (
              {h.settings.balanceDays
                ? `last ${h.settings.balanceDays} days`
                : "all time"}
              )
            </small>
          </p>
        </div>
      </div>
      <div className="history-controls">
        <div className="chips">
          {[
            ["all", "Everyone"],
            ["a", h.settings.names[0]],
            ["b", h.settings.names[1]],
          ].map(([id, label]) => (
            <button
              key={id}
              className={filter === id ? "selected" : ""}
              onClick={() => {
                setFilter(id);
                setLimit(50);
              }}
            >
              {label}
            </button>
          ))}
        </div>
        <input
          type="search"
          placeholder="Search history"
          aria-label="Search history"
          value={query}
          onChange={(e) => {
            setQuery(e.target.value);
            setLimit(50);
          }}
        />
      </div>
      <section className="history-list">
        {!entries.length && (
          <div className="empty">
            <History size={32} />
            <h2>A fresh page</h2>
            <p>Completed chores will appear here.</p>
          </div>
        )}
        {entries.slice(0, limit).map((c) => (
          <div key={c.id} className={`history-row ${c.undone ? "undone" : ""}`}>
            <span className={`history-icon ${c.person}`}>
              <ChoreIcon value={c.icon} size={23} />
            </span>
            <div className="history-text">
              <strong>{c.name}</strong>
              <span>
                {personName(h, c.person)} · {formatDate(c.date, true)}
                {c.undone ? " · Removed" : ""}
              </span>
              {c.note && <p>{c.note}</p>}
            </div>
            <strong className="history-points">
              {c.undone ? "—" : `+${c.points}`}
              <small>pts</small>
            </strong>
            {c.person === person && !c.undone && (
              <button
                className="icon-button"
                aria-label={`Undo ${c.name} on ${c.date}`}
                disabled={disabled}
                onClick={() => setRemove(c.id)}
              >
                <RotateCcw size={18} />
              </button>
            )}
          </div>
        ))}
      </section>
      {entries.length > limit && (
        <button className="secondary full" onClick={() => setLimit(limit + 50)}>
          Show more
        </button>
      )}
      {remove && (
        <Modal
          title="Remove this completion?"
          description="The points will be deducted. The original entry stays in history as removed."
          onClose={() => setRemove(null)}
        >
          <div className="actions">
            <button className="secondary" onClick={() => setRemove(null)}>
              Keep it
            </button>
            <button
              className="primary"
              disabled={disabled}
              onClick={async () => {
                if (await mutate({ type: "undo", id: remove })) setRemove(null);
              }}
            >
              Remove completion
            </button>
          </div>
        </Modal>
      )}
    </>
  );
}
function SettingsView({
  h,
  person,
  mutate,
  disabled,
  onRecovery,
  onRestore,
}: Omit<Shared, "today"> & {
  onRecovery: (r: any) => void;
  onRestore: (s: Snapshot) => void;
}) {
  const [editing, setEditing] = useState<Chore | null>(null);
  const [settings, setSettings] = useState<Settings>(h.settings);
  const [settingsBase, setSettingsBase] = useState(h.settings);
  const [dirty, setDirty] = useState(false);
  const [localError, setLocalError] = useState("");
  const [current, setCurrent] = useState("");
  const [password, setPassword] = useState("");
  const [passwordBusy, setPasswordBusy] = useState(false);
  const [restoreData, setRestoreData] = useState<any>(null);
  const [restoreBusy, setRestoreBusy] = useState(false);
  const [query, setQuery] = useState("");
  useEffect(() => {
    if (!dirty) {
      setSettings(h.settings);
      setSettingsBase(h.settings);
    }
  }, [h.settings, dirty]);
  const update = (patch: Partial<Settings>) => {
    setSettings({ ...settings, ...patch });
    setDirty(true);
  };
  const locked =
    h.swaps.length > 0 || h.completions.some((c) => c.choreId === "bedtime");
  return (
    <>
      <div className="page-heading">
        <div>
          <span className="eyebrow">Make it yours</span>
          <h1>A home that works for you.</h1>
          <p>Agree on the effort. Adjust as life changes.</p>
        </div>
      </div>
      <section className="settings-panel">
        <div className="section-heading">
          <h2>Household & balance</h2>
        </div>
        <form
          onSubmit={async (e) => {
            e.preventDefault();
            if (
              await mutate({
                type: "saveSettings",
                settings,
                expected: settingsBase,
              })
            )
              setDirty(false);
          }}
        >
          <div className="form-grid">
            <Field label="Household name">
              <input
                required
                maxLength={80}
                value={settings.householdName}
                onChange={(e) => update({ householdName: e.target.value })}
              />
            </Field>
            <Field label="Rolling balance">
              <select
                value={settings.balanceDays}
                onChange={(e) =>
                  update({ balanceDays: Number(e.target.value) })
                }
              >
                {[7, 14, 30, 60, 90, 365, 0].map((n) => (
                  <option key={n} value={n}>
                    {n ? `Last ${n} days` : "All time"}
                  </option>
                ))}
              </select>
            </Field>
            {[0, 1].map((i) => (
              <Field label={`Adult ${i + 1} name`} key={i}>
                <input
                  required
                  maxLength={40}
                  value={settings.names[i]}
                  onChange={(e) => {
                    const names = [...settings.names] as [string, string];
                    names[i] = e.target.value;
                    update({ names });
                  }}
                />
              </Field>
            ))}
          </div>
          <details className="settings-disclosure">
            <summary>Bedtime starting pattern</summary>
            <p className="hint">
              {locked
                ? "The starting pattern is preserved after the first bedtime or swap. Use the calendar to exchange individual nights."
                : "You can change the starting pattern until the first bedtime is recorded or swapped."}
            </p>
            <div className="form-grid">
              <Field label="Starting date">
                <input
                  type="date"
                  required
                  disabled={locked}
                  value={settings.bedtimeStart}
                  onChange={(e) => update({ bedtimeStart: e.target.value })}
                />
              </Field>
              <Field label="First night belongs to">
                <select
                  disabled={locked}
                  value={settings.bedtimeFirst}
                  onChange={(e) =>
                    update({ bedtimeFirst: e.target.value as Person })
                  }
                >
                  <option value="a">{settings.names[0]}</option>
                  <option value="b">{settings.names[1]}</option>
                </select>
              </Field>
            </div>
          </details>
          <div className="actions">
            <button
              type="button"
              className="secondary"
              disabled={!dirty}
              onClick={() => {
                setSettings(h.settings);
                setSettingsBase(h.settings);
                setDirty(false);
              }}
            >
              Reset form
            </button>
            <button className="primary" disabled={disabled || !dirty}>
              Save household settings
            </button>
          </div>
        </form>
      </section>
      <section className="settings-panel">
        <div className="section-heading">
          <div>
            <h2>Chores & point values</h2>
            <p className="hint">
              New values apply to future completions. Archived chores keep their
              history.
            </p>
          </div>
          <button
            className="secondary"
            disabled={disabled}
            onClick={() =>
              setEditing({
                id: crypto.randomUUID(),
                name: "",
                icon: "custom",
                category: "Everyday",
                points: 3,
                intervalDays: null,
                archived: false,
              })
            }
          >
            <Plus size={18} />
            Add chore
          </button>
        </div>
        <input
          type="search"
          placeholder="Find a chore"
          aria-label="Find a chore"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        <div className="managed-list">
          {h.chores
            .filter((c) => c.name.toLowerCase().includes(query.toLowerCase()))
            .map((c) => (
              <button
                className={`managed-row ${c.archived ? "archived" : ""}`}
                key={c.id}
                disabled={disabled}
                onClick={() => setEditing(c)}
              >
                <span className="managed-icon">
                  <ChoreIcon value={c.icon} size={23} />
                </span>
                <span>
                  <strong>{c.name}</strong>
                  <small>
                    {c.archived ? "Archived" : c.category}
                    {c.intervalDays ? ` · Every ${c.intervalDays} days` : ""}
                  </small>
                </span>
                <b>{c.points} pts</b>
                <Settings2 size={17} />
              </button>
            ))}
        </div>
      </section>
      <section className="settings-panel">
        <h2>Your account</h2>
        <p className="hint">
          Signed in as {personName(h, person)}. Each account records its own
          contributions.
        </p>
        <details className="settings-disclosure">
          <summary>Change password & recovery code</summary>
          <form
            onSubmit={async (e) => {
              e.preventDefault();
              setPasswordBusy(true);
              setLocalError("");
              try {
                const r = await api("/auth/password", { current, password });
                onRecovery(r.recoveryCodes);
                setCurrent("");
                setPassword("");
              } catch (e) {
                setLocalError((e as Error).message);
              } finally {
                setPasswordBusy(false);
              }
            }}
          >
            <Field label="Current password">
              <input
                type="password"
                required
                autoComplete="current-password"
                value={current}
                onChange={(e) => setCurrent(e.target.value)}
              />
            </Field>
            <Field label="New password">
              <input
                type="password"
                required
                minLength={12}
                maxLength={256}
                autoComplete="new-password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
              />
            </Field>
            <p className="hint">
              This signs out your other devices and replaces your recovery code.
            </p>
            <button className="primary" disabled={disabled || passwordBusy}>
              Change password
            </button>
          </form>
        </details>
        <button
          className="text-button mobile-signout"
          disabled={disabled}
          onClick={async () => {
            try {
              await api("/auth/logout", {});
              location.reload();
            } catch (e) {
              setLocalError((e as Error).message);
            }
          }}
        >
          <LogOut size={17} />
          Sign out
        </button>
      </section>
      <section className="settings-panel">
        <h2>Keep a copy</h2>
        <p className="hint">
          Download your household’s chores, points history and bedtime swaps.
          Account passwords are excluded.
        </p>
        <div className="backup-actions">
          <a className="secondary" href="/api/backup" download>
            <Download size={17} />
            Download backup
          </a>
          {person === "a" && (
            <label
              className={`secondary file-button ${disabled ? "disabled" : ""}`}
            >
              Restore backup
              <input
                type="file"
                accept=".json,application/json"
                disabled={disabled}
                onChange={async (e) => {
                  const file = e.target.files?.[0];
                  if (!file) return;
                  setLocalError("");
                  try {
                    if (file.size > 16 * 1024 * 1024)
                      throw new Error("Backup must be smaller than 16 MB.");
                    const data = JSON.parse(await file.text());
                    if (data.version !== 1 || !data.household)
                      throw new Error("Choose a Chores backup file.");
                    setRestoreData(data);
                  } catch (e) {
                    setLocalError((e as Error).message);
                  }
                  e.target.value = "";
                }}
              />
            </label>
          )}
        </div>
      </section>
      {localError && (
        <p className="error" role="alert">
          {localError}
        </p>
      )}
      {editing && (
        <EditChore
          chore={editing}
          expected={h.chores.find((c) => c.id === editing.id) ?? null}
          mutate={mutate}
          disabled={disabled}
          onClose={() => setEditing(null)}
        />
      )}{" "}
      {restoreData && (
        <Modal
          title="Replace household data?"
          description="This replaces chores, history and the bedtime calendar for both people. Download a current backup first. Account passwords stay the same."
          onClose={() => setRestoreData(null)}
        >
          <div className="actions">
            <button className="secondary" onClick={() => setRestoreData(null)}>
              Cancel
            </button>
            <button
              className="primary"
              disabled={disabled || restoreBusy}
              onClick={async () => {
                setRestoreBusy(true);
                try {
                  const s = await api("/restore", {
                    ...restoreData,
                    revision: h.revision,
                  });
                  onRestore(s);
                  setRestoreData(null);
                } catch (e) {
                  setLocalError((e as Error).message);
                  setRestoreData(null);
                } finally {
                  setRestoreBusy(false);
                }
              }}
            >
              Replace household data
            </button>
          </div>
        </Modal>
      )}
    </>
  );
}
function EditChore({
  chore: initial,
  expected,
  mutate,
  disabled,
  onClose,
}: {
  chore: Chore;
  expected: Chore | null;
  mutate: Shared["mutate"];
  disabled: boolean;
  onClose: () => void;
}) {
  const [chore, setChore] = useState(initial);
  const expectedBase = useRef(expected).current;
  return (
    <Modal
      title={initial.name ? "Edit chore" : "Add a chore"}
      description="Choose a point value that feels fair to both of you."
      onClose={onClose}
    >
      <form
        onSubmit={async (e) => {
          e.preventDefault();
          if (
            await mutate({ type: "saveChore", chore, expected: expectedBase })
          )
            onClose();
        }}
      >
        <Field label="Chore name">
          <input
            required
            maxLength={80}
            value={chore.name}
            onChange={(e) => setChore({ ...chore, name: e.target.value })}
          />
        </Field>
        <div className="form-grid">
          <Field label="Points">
            <input
              type="number"
              required
              min={0}
              max={1000}
              step={1}
              value={chore.points}
              onChange={(e) =>
                setChore({ ...chore, points: Number(e.target.value) })
              }
            />
          </Field>
          <Field label="Category">
            <select
              disabled={chore.id === "bedtime"}
              value={chore.category}
              onChange={(e) => setChore({ ...chore, category: e.target.value })}
            >
              {[
                "Everyday",
                "Laundry",
                "Maintenance",
                ...(chore.id === "bedtime" ? ["Bedtime"] : []),
              ].map((c) => (
                <option key={c}>{c}</option>
              ))}
            </select>
          </Field>
        </div>
        <Field label="Icon">
          <select
            value={chore.icon}
            onChange={(e) => setChore({ ...chore, icon: e.target.value })}
          >
            {Object.keys(icons).map((c) => (
              <option key={c} value={c}>
                {
                  (
                    {
                      dishwasher: "Dishes",
                      washer: "Washing machine",
                      hang: "Drying",
                      fold: "Clothing",
                      tidy: "Sparkles",
                      bedtime: "Moon",
                      robot: "Robot",
                      filter: "Filter",
                      drain: "Drain",
                      custom: "Checkmark",
                    } as Record<string, string>
                  )[c]
                }
              </option>
            ))}
          </select>
        </Field>
        {chore.id !== "bedtime" && (
          <>
            <Field label="Repeat interval in days (optional)">
              <input
                type="number"
                min={1}
                max={3650}
                step={1}
                placeholder="For example, 90 for HVAC filters"
                value={chore.intervalDays ?? ""}
                onChange={(e) =>
                  setChore({
                    ...chore,
                    intervalDays: e.target.value
                      ? Number(e.target.value)
                      : null,
                  })
                }
              />
            </Field>
            <label className="checkbox">
              <input
                type="checkbox"
                checked={chore.archived}
                onChange={(e) =>
                  setChore({ ...chore, archived: e.target.checked })
                }
              />
              Archive this chore
            </label>
          </>
        )}
        <div className="actions">
          <button className="secondary" type="button" onClick={onClose}>
            Cancel
          </button>
          <button className="primary" disabled={disabled}>
            Save chore
          </button>
        </div>
      </form>
    </Modal>
  );
}
createRoot(document.getElementById("root")!).render(<App />);
if ("serviceWorker" in navigator && import.meta.env.PROD)
  navigator.serviceWorker.register("/sw.js").catch(() => {});
