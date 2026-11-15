import { locale, t } from "./i18n.svelte";

const DAY = 86_400;

export function formatPlaytime(secs: number): string {
  if (secs <= 0) return "–";
  const h = Math.floor(secs / 3600);
  const m = Math.round((secs % 3600) / 60);
  if (h === 0) return t("{m} Min.", { m: Math.max(m, 1) });
  if (h < 10 && m > 0) return t("{h} Std. {m} Min.", { h, m });
  return t("{h} Std.", { h });
}

export function formatDuration(secs: number): string {
  const h = Math.floor(secs / 3600);
  const m = Math.round((secs % 3600) / 60);
  return h > 0 ? t("{h} Std. {m} Min.", { h, m }) : t("{m} Min.", { m: Math.max(m, 1) });
}

export function relativeDay(ts: number | null, now = Date.now() / 1000): string {
  if (!ts) return t("Noch nie");
  const startOfToday = new Date();
  startOfToday.setHours(0, 0, 0, 0);
  const today = startOfToday.getTime() / 1000;
  if (ts >= today) return t("Heute");
  if (ts >= today - DAY) return t("Gestern");
  const days = Math.floor((now - ts) / DAY);
  if (days < 7) return t("vor {n} Tagen", { n: days });
  if (days < 14) return t("vor 1 Woche");
  if (days < 31) return t("vor {n} Wochen", { n: Math.floor(days / 7) });
  if (days < 60) return t("vor 1 Monat");
  if (days < 365) return t("vor {n} Monaten", { n: Math.floor(days / 30) });
  return days < 730 ? t("vor 1 Jahr") : t("vor {n} Jahren", { n: Math.floor(days / 365) });
}

export function formatDate(ts: number): string {
  return new Date(ts * 1000).toLocaleDateString(locale(), { day: "numeric", month: "short", year: "numeric" });
}

export function formatSessionRange(start: number, end: number): string {
  const s = new Date(start * 1000);
  const e = new Date(end * 1000);
  const day = relativeDay(start);
  const dayLabel =
    day === t("Heute") || day === t("Gestern")
      ? day
      : s.toLocaleDateString(locale(), { weekday: "short", day: "numeric", month: "short" });
  const time = (d: Date) => d.toLocaleTimeString(locale(), { hour: "2-digit", minute: "2-digit" });
  return `${dayLabel}, ${time(s)} – ${time(e)}`;
}

/** Deterministic placeholder colours for games without a cover. */
export function placeholderColors(name: string): [string, string] {
  let h = 0;
  for (const c of name) h = (h * 31 + c.charCodeAt(0)) >>> 0;
  const hue = h % 360;
  return [`hsl(${hue} 45% 32%)`, `hsl(${(hue + 20) % 360} 50% 8%)`];
}

/** Playtime per day for the last `days` days (oldest first). */
export function dailyPlaytime(
  sessions: { startedAt: number; endedAt: number }[],
  days = 30,
): { day: Date; secs: number }[] {
  const start = new Date();
  start.setHours(0, 0, 0, 0);
  start.setDate(start.getDate() - (days - 1));
  const buckets = Array.from({ length: days }, (_, i) => {
    const d = new Date(start);
    d.setDate(start.getDate() + i);
    return { day: d, secs: 0 };
  });
  for (const s of sessions) {
    // Split sessions across midnight.
    let from = s.startedAt;
    while (from < s.endedAt) {
      const d = new Date(from * 1000);
      d.setHours(24, 0, 0, 0);
      const to = Math.min(s.endedAt, d.getTime() / 1000);
      const idx = Math.floor((from * 1000 - start.getTime()) / (DAY * 1000));
      if (idx >= 0 && idx < days) buckets[idx].secs += to - from;
      from = to;
    }
  }
  return buckets;
}
