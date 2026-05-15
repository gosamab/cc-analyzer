// Global UI state. Svelte 5 runes; keep this thin.

type View = "dashboard" | "explorer" | "insights" | "settings";
type Range = 1 | 7 | 30 | 90;

class UIState {
  view = $state<View>("dashboard");
  range = $state<Range>(7);
  selectedSession = $state<string | null>(null);
  projectFilter = $state<string | null>(null);

  open(view: View) {
    this.view = view;
  }
  drillSession(id: string) {
    this.selectedSession = id;
    this.view = "explorer";
  }
  drillProject(p: string) {
    this.projectFilter = p;
    this.view = "explorer";
  }
}

export const ui = new UIState();

// Display currency. Costs are stored in USD; this multiplier converts at render time.
class CurrencyState {
  code = $state<string>("USD");
  // USD → target multiplier. 1.0 = USD.
  rate = $state<number>(1);

  load() {
    try {
      const raw = localStorage.getItem("cc.currency");
      if (raw) {
        const { code, rate } = JSON.parse(raw) as { code: string; rate: number };
        if (code) this.code = code;
        if (rate > 0) this.rate = rate;
      }
    } catch {
      // ignore — defaults remain
    }
  }

  set(code: string, rate: number) {
    this.code = code || "USD";
    this.rate = rate > 0 ? rate : 1;
    try {
      localStorage.setItem("cc.currency", JSON.stringify({ code: this.code, rate: this.rate }));
    } catch {
      // ignore
    }
  }
}

export const currency = new CurrencyState();

type ThemeMode = "dark" | "light";

class ThemeState {
  mode = $state<ThemeMode>("dark");

  load() {
    try {
      const saved = localStorage.getItem("cc.theme");
      if (saved === "light" || saved === "dark") this.mode = saved;
    } catch {
      // ignore
    }
    this.apply();
  }

  set(m: ThemeMode) {
    this.mode = m;
    try {
      localStorage.setItem("cc.theme", m);
    } catch {
      // ignore
    }
    this.apply();
  }

  toggle() {
    this.set(this.mode === "dark" ? "light" : "dark");
  }

  private apply() {
    if (typeof document === "undefined") return;
    document.documentElement.classList.toggle("light", this.mode === "light");
  }
}

export const theme = new ThemeState();
