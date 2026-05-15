import { currency } from "./store.svelte";

const intFmt = new Intl.NumberFormat("en-US");
const decFmt = (d: number) =>
  new Intl.NumberFormat("en-US", {
    minimumFractionDigits: d,
    maximumFractionDigits: d,
  });

// Cache formatters per currency code to avoid recreating Intl objects on every call.
const moneyFmtCache = new Map<string, Intl.NumberFormat>();
function moneyFmt(code: string) {
  let f = moneyFmtCache.get(code);
  if (!f) {
    try {
      f = new Intl.NumberFormat("en-US", {
        style: "currency",
        currency: code,
        minimumFractionDigits: 2,
        maximumFractionDigits: 2,
      });
    } catch {
      f = new Intl.NumberFormat("en-US", {
        style: "currency",
        currency: "USD",
        minimumFractionDigits: 2,
        maximumFractionDigits: 2,
      });
    }
    moneyFmtCache.set(code, f);
  }
  return f;
}

// Reads from the reactive currency store, so values re-format when the user changes it.
export const fmtUsd = (n: number) => moneyFmt(currency.code).format((n ?? 0) * currency.rate);

// Abbreviated money for tight columns — keep the currency prefix, suffix with k/M/B.
export const fmtUsdShort = (n: number) => {
  const v = (n ?? 0) * currency.rate;
  const code = currency.code;
  const prefix = code === "USD" ? "$" : `${code} `;
  const abs = Math.abs(v);
  if (abs >= 1_000_000_000) return `${prefix}${fmtDec(v / 1_000_000_000, 2)}B`;
  if (abs >= 1_000_000) return `${prefix}${fmtDec(v / 1_000_000, 2)}M`;
  if (abs >= 1_000) return `${prefix}${fmtDec(v / 1_000, 1)}k`;
  return moneyFmt(code).format(v);
};

export const fmtInt = (n: number) => intFmt.format(n ?? 0);

export const fmtDec = (n: number, d = 2) => decFmt(d).format(n ?? 0);

export const fmtTok = (n: number) => {
  if (n >= 1_000_000_000) return `${fmtDec(n / 1_000_000_000, 2)}B`;
  if (n >= 1_000_000) return `${fmtDec(n / 1_000_000, 2)}M`;
  if (n >= 1_000) return `${fmtDec(n / 1_000, 2)}k`;
  return fmtInt(n);
};

export const fmtPct = (n: number, d = 2) => `${fmtDec(n, d)}%`;

// Human-readable duration. Accepts minutes (can be fractional).
export const fmtDuration = (minutes: number) => {
  const m = Math.max(0, Math.round(minutes ?? 0));
  if (m < 1) return "<1m";
  if (m < 60) return `${m}m`;
  const days = Math.floor(m / 1440);
  const hours = Math.floor((m % 1440) / 60);
  const mins = m % 60;
  if (days > 0) return hours > 0 ? `${days}d ${hours}h` : `${days}d`;
  return mins > 0 ? `${hours}h ${mins}m` : `${hours}h`;
};

export const today = () => new Date().toISOString().slice(0, 10);

export const daysAgo = (n: number) => {
  const d = new Date();
  d.setUTCDate(d.getUTCDate() - n);
  return d.toISOString().slice(0, 10);
};

export const isoRange = (days: number) => ({
  since: `${daysAgo(days - 1)}T00:00:00`,
  until: `${today()}T23:59:59`,
});

export const shortProject = (p: string) => {
  const stripped = p.replace(/^-Users-[^-]+-?/, "").replace(/-/g, "/");
  const parts = stripped.split("/").filter(Boolean);
  if (parts.length <= 2) return stripped || p;
  return ".../" + parts.slice(-2).join("/");
};

export const shortSession = (s: string) => s.slice(0, 8);
