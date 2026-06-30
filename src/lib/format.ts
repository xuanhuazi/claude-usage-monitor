/** Compact token count: 12_345 -> "12.3K", 4_400_000 -> "4.4M". */
export function fmtTokens(n: number): string {
  const v = Math.max(0, Math.round(n));
  if (v >= 1_000_000_000) return (v / 1_000_000_000).toFixed(2) + "B";
  if (v >= 1_000_000) return (v / 1_000_000).toFixed(2) + "M";
  if (v >= 1_000) return (v / 1_000).toFixed(1) + "K";
  return String(v);
}

export function fmtPct(frac: number): string {
  return (frac * 100).toFixed(frac >= 0.1 ? 0 : 1) + "%";
}

/** Equivalent USD cost. */
export function fmtUsd(n: number): string {
  if (n >= 100) return `$${n.toFixed(0)}`;
  if (n > 0 && n < 0.01) return `$${n.toFixed(4)}`;
  return `$${n.toFixed(2)}`;
}

/** Seconds -> "2小时15分" / "8分" / "—". */
export function fmtDuration(secs: number | null): string {
  if (secs == null || secs < 0) return "—";
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  if (h > 0) return `${h}小时${m}分`;
  if (m > 0) return `${m}分`;
  return `${secs}秒`;
}

/** Absolute clock time for a reset, e.g. "今天 18:30" / "明天 02:00". */
export function fmtResetTime(ms: number | null): string {
  if (ms == null) return "—";
  const d = new Date(ms);
  const now = new Date();
  const sameDay = d.toDateString() === now.toDateString();
  const tomorrow = new Date(now.getTime() + 86_400_000);
  const isTomorrow = d.toDateString() === tomorrow.toDateString();
  const hh = String(d.getHours()).padStart(2, "0");
  const mm = String(d.getMinutes()).padStart(2, "0");
  const prefix = sameDay ? "今天 " : isTomorrow ? "明天 " : `${d.getMonth() + 1}/${d.getDate()} `;
  return `${prefix}${hh}:${mm}`;
}

export function fmtDate(ms: number): string {
  const d = new Date(ms);
  return `${d.getMonth() + 1}/${d.getDate()}`;
}

export function fmtDateTime(ms: number): string {
  const d = new Date(ms);
  const hh = String(d.getHours()).padStart(2, "0");
  const mm = String(d.getMinutes()).padStart(2, "0");
  return `${fmtDate(ms)} ${hh}:${mm}`;
}

/** Pick a status color token name based on utilization fraction. */
export function statusColor(pct: number): "success" | "warn" | "danger" {
  if (pct >= 0.9) return "danger";
  if (pct >= 0.75) return "warn";
  return "success";
}
