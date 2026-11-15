const DAY = 86_400;

export function formatPlaytime(secs: number): string {
  if (secs <= 0) return "–";
  const h = Math.floor(secs / 3600);
  const m = Math.round((secs % 3600) / 60);
  if (h === 0) return `${Math.max(m, 1)} Min.`;
  if (h < 10 && m > 0) return `${h} Std. ${m} Min.`;
  return `${h} Std.`;
}

export function formatDuration(secs: number): string {
  const h = Math.floor(secs / 3600);
  const m = Math.round((secs % 3600) / 60);
  return h > 0 ? `${h} Std. ${m} Min.` : `${Math.max(m, 1)} Min.`;
}

export function relativeDay(ts: number | null, now = Date.now() / 1000): string {
  if (!ts) return "Noch nie";
  const startOfToday = new Date();
  startOfToday.setHours(0, 0, 0, 0);
  const today = startOfToday.getTime() / 1000;
  if (ts >= today) return "Heute";
  if (ts >= today - DAY) return "Gestern";
  const days = Math.floor((now - ts) / DAY);
  if (days < 7) return `vor ${days} Tagen`;
  if (days < 14) return "vor 1 Woche";
  if (days < 31) return `vor ${Math.floor(days / 7)} Wochen`;
  if (days < 60) return "vor 1 Monat";
  if (days < 365) return `vor ${Math.floor(days / 30)} Monaten`;
  return days < 730 ? "vor 1 Jahr" : `vor ${Math.floor(days / 365)} Jahren`;
}

export function formatDate(ts: number): string {
  return new Date(ts * 1000).toLocaleDateString("de-DE", { day: "numeric", month: "short", year: "numeric" });
}

export function formatSessionRange(start: number, end: number): string {
  const s = new Date(start * 1000);
  const e = new Date(end * 1000);
  const day = relativeDay(start);
  const dayLabel =
    day === "Heute" || day === "Gestern"
      ? day
      : s.toLocaleDateString("de-DE", { weekday: "short", day: "numeric", month: "short" });
  const t = (d: Date) => d.toLocaleTimeString("de-DE", { hour: "2-digit", minute: "2-digit" });
  return `${dayLabel}, ${t(s)} – ${t(e)}`;
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
