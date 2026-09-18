// Pure formatting helpers (no DOM, no state): unit-tested, shared by the
// session list and the context meter.

/** "14:32" today, "12 ene" otherwise; "" for garbage input. */
export function timeOf(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "";
  const now = new Date();
  if (d.toDateString() === now.toDateString()) {
    return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  }
  return d.toLocaleDateString([], { day: "2-digit", month: "short" });
}

/** Compact thousands for the context meter: 950 → "950", 8200 → "8.2K". */
export function fmtK(n: number): string {
  return n >= 1000 ? `${(n / 1000).toFixed(1)}K` : `${n}`;
}

/** Context usage 0–100 for the meter width. */
export function contextPct(used: number, window: number | null): number {
  if (window == null || window <= 0) return 0;
  return Math.min(100, (used / window) * 100);
}
