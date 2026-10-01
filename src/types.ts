export type Person = "a" | "b";
export type Chore = {
  id: string;
  name: string;
  icon: string;
  category: string;
  points: number;
  intervalDays: number | null;
  archived: boolean;
};
export type Settings = {
  householdName: string;
  names: [string, string];
  balanceDays: number;
  bedtimeStart: string;
  bedtimeFirst: Person;
};
export type Completion = {
  id: string;
  choreId: string;
  name: string;
  icon: string;
  person: Person;
  points: number;
  date: string;
  note: string;
  createdAt: string;
  undone: boolean;
};
export type Swap = {
  id: string;
  first: string;
  second: string;
  firstPerson: Person;
  secondPerson: Person;
  actor: Person;
  createdAt: string;
};
export type Household = {
  revision: number;
  settings: Settings;
  chores: Chore[];
  completions: Completion[];
  overrides: Record<string, Person>;
  swaps: Swap[];
};
export type Snapshot = { person: Person; household: Household; today: string };
export type Operation =
  | {
      type: "complete";
      choreId: string;
      expectedPoints: number;
      date: string;
      note: string;
    }
  | { type: "undo"; id: string }
  | { type: "saveChore"; chore: Chore; expected: Chore | null }
  | { type: "saveSettings"; settings: Settings; expected: Settings }
  | { type: "swap"; first: string; second: string };
export function addDays(date: string, days: number) {
  const d = new Date(`${date}T12:00:00Z`);
  d.setUTCDate(d.getUTCDate() + days);
  return d.toISOString().slice(0, 10);
}
export function assignee(h: Household, date: string): Person | null {
  if (date < h.settings.bedtimeStart) return null;
  const days = Math.round(
    (Date.parse(`${date}T12:00:00Z`) -
      Date.parse(`${h.settings.bedtimeStart}T12:00:00Z`)) /
      86400000,
  );
  return (
    h.overrides[date] ??
    (days % 2 === 0
      ? h.settings.bedtimeFirst
      : h.settings.bedtimeFirst === "a"
        ? "b"
        : "a")
  );
}
export function personName(h: Household, p: Person) {
  return h.settings.names[p === "a" ? 0 : 1];
}
export function formatDate(date: string, short = false) {
  return new Intl.DateTimeFormat("en-GB", {
    day: "numeric",
    month: short ? "short" : "long",
    timeZone: "UTC",
  }).format(new Date(`${date}T12:00:00Z`));
}
