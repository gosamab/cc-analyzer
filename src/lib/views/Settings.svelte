<script lang="ts">
  import { ipc, type CacheStats, type PricingRow } from "../ipc";
  import { currency, theme } from "../store.svelte";
  import { fmtInt, fmtUsd } from "../format";

  // Currency presets — USD→target rates are user-editable below.
  const currencyPresets: Record<string, number> = {
    USD: 1,
    SAR: 3.76,
    EUR: 0.92,
    GBP: 0.79,
    AED: 3.67,
    JPY: 156,
    INR: 83,
  };
  let currencyCode = $state(currency.code);
  let currencyRate = $state(currency.rate);

  function applyCurrency() {
    currency.set(currencyCode, currencyRate);
  }
  function pickPreset(code: string) {
    currencyCode = code;
    currencyRate = currencyPresets[code] ?? 1;
    applyCurrency();
  }

  let stats = $state<CacheStats | null>(null);
  let pricing = $state<PricingRow[]>([]);
  let pricingDraft = $state<PricingRow[]>([]);
  let loadingStats = $state(true);
  let loadingPricing = $state(true);
  let clearing = $state(false);
  let saving = $state(false);
  let saveMsg = $state<string>("");

  function load() {
    loadingStats = true;
    loadingPricing = true;
    ipc.cacheStats()
      .then((s) => (stats = s))
      .catch(console.error)
      .finally(() => (loadingStats = false));
    ipc.pricingTable()
      .then((p) => {
        pricing = p;
        pricingDraft = p.map((r) => ({ ...r }));
      })
      .catch(console.error)
      .finally(() => (loadingPricing = false));
  }

  const pricingDirty = $derived(
    pricingDraft.some((r, i) => {
      const o = pricing[i];
      if (!o) return true;
      return (
        r.input !== o.input ||
        r.output !== o.output ||
        r.cache_write !== o.cache_write ||
        r.cache_read !== o.cache_read
      );
    }),
  );

  async function savePricing() {
    saving = true;
    saveMsg = "";
    try {
      const updated = await ipc.setPricing(pricingDraft);
      pricing = pricingDraft.map((r) => ({ ...r }));
      saveMsg = `Saved — re-priced ${updated} rows.`;
      load();
    } catch (e) {
      saveMsg = `Save failed: ${e}`;
      console.error(e);
    } finally {
      saving = false;
    }
  }

  function resetPricing() {
    pricingDraft = pricing.map((r) => ({ ...r }));
    saveMsg = "";
  }

  $effect(() => {
    load();
  });

  async function onClearCache() {
    const ok = confirm(
      "Wipe the local cache? Next refresh will re-ingest every JSONL file from scratch (can take a few minutes).",
    );
    if (!ok) return;
    clearing = true;
    try {
      await ipc.clearCache();
      await ipc.refreshLogs();
      load();
    } catch (e) {
      console.error(e);
    } finally {
      clearing = false;
    }
  }

  function fmtMB(bytes: number) {
    return `${(bytes / 1_048_576).toFixed(1)} MB`;
  }
  function fmtDate(ts: string | null) {
    return ts ? ts.slice(0, 10) : "—";
  }
</script>

<div class="p-6 space-y-4 max-w-3xl">
  <h1 class="text-lg font-semibold">Settings</h1>

  <!-- Theme -->
  <div class="card space-y-3">
    <div class="card-title">Appearance</div>
    <div class="flex gap-2">
      <button
        class="px-3 py-1 text-sm rounded border {theme.mode === 'dark'
          ? 'border-accent text-ink bg-panel2'
          : 'border-border text-muted hover:text-ink'}"
        onclick={() => theme.set('dark')}
      >
        ☾ Dark
      </button>
      <button
        class="px-3 py-1 text-sm rounded border {theme.mode === 'light'
          ? 'border-accent text-ink bg-panel2'
          : 'border-border text-muted hover:text-ink'}"
        onclick={() => theme.set('light')}
      >
        ☀ Light
      </button>
    </div>
  </div>

  <!-- Currency -->
  <div class="card space-y-3">
    <div class="card-title">Display currency</div>
    <div class="text-xs text-muted">
      Costs are stored in USD. This converts on display only — change the rate any time.
    </div>
    <div class="flex flex-wrap gap-2">
      {#each Object.keys(currencyPresets) as code}
        <button
          class="px-2 py-1 text-xs rounded border {currencyCode === code
            ? 'border-accent text-ink bg-panel2'
            : 'border-border text-muted hover:text-ink'}"
          onclick={() => pickPreset(code)}
        >
          {code}
        </button>
      {/each}
    </div>
    <div class="flex items-center gap-3 text-sm">
      <label class="flex items-center gap-2">
        <span class="text-muted text-xs uppercase tracking-wide">Code</span>
        <input
          type="text"
          maxlength="3"
          class="w-20 bg-panel2 border border-border rounded px-2 py-1 uppercase num"
          bind:value={currencyCode}
        />
      </label>
      <label class="flex items-center gap-2">
        <span class="text-muted text-xs uppercase tracking-wide">USD →</span>
        <input
          type="number"
          step="0.0001"
          min="0"
          class="w-28 bg-panel2 border border-border rounded px-2 py-1 num"
          bind:value={currencyRate}
        />
      </label>
      <button class="btn text-xs" onclick={applyCurrency}>Apply</button>
      <div class="ml-auto text-xs text-muted">
        Preview: $1.00 → <span class="num text-ink">{fmtUsd(1)}</span>
      </div>
    </div>
  </div>

  <!-- Data location -->
  <div class="card space-y-2">
    <div class="card-title">Data location</div>
    <div class="text-sm text-muted">
      Reading from <code class="text-ink">~/.claude/projects/</code>.
    </div>
    <div class="text-sm text-muted">
      Cache at <code class="text-ink">~/Library/Application Support/cc-analyzer/cache.db</code>.
    </div>
  </div>

  <!-- Cache stats -->
  <div class="card space-y-3">
    <div class="card-title flex items-center justify-between">
      <span>Local cache</span>
      <button class="btn text-xs" onclick={onClearCache} disabled={clearing}>
        {clearing ? "Clearing…" : "Clear & re-ingest"}
      </button>
    </div>
    {#if loadingStats && !stats}
      <div class="text-sm text-muted">Loading…</div>
    {:else if stats}
      <div class="grid grid-cols-3 gap-3 text-sm">
        <div>
          <div class="text-xs text-muted uppercase tracking-wide">Messages</div>
          <div class="num text-lg">{fmtInt(stats.messages)}</div>
        </div>
        <div>
          <div class="text-xs text-muted uppercase tracking-wide">Sessions</div>
          <div class="num text-lg">{fmtInt(stats.sessions)}</div>
        </div>
        <div>
          <div class="text-xs text-muted uppercase tracking-wide">Projects</div>
          <div class="num text-lg">{fmtInt(stats.projects)}</div>
        </div>
        <div>
          <div class="text-xs text-muted uppercase tracking-wide">First seen</div>
          <div class="num">{fmtDate(stats.first_ts)}</div>
        </div>
        <div>
          <div class="text-xs text-muted uppercase tracking-wide">Last seen</div>
          <div class="num">{fmtDate(stats.last_ts)}</div>
        </div>
        <div>
          <div class="text-xs text-muted uppercase tracking-wide">DB size</div>
          <div class="num">{fmtMB(stats.db_bytes)}</div>
        </div>
      </div>
    {/if}
  </div>

  <!-- Pricing -->
  <div class="card space-y-3">
    <div class="card-title flex items-center justify-between">
      <span>Pricing (USD per 1M tokens)</span>
      <div class="flex items-center gap-2">
        {#if saveMsg}
          <span class="text-xs text-muted">{saveMsg}</span>
        {/if}
        {#if pricingDirty}
          <button class="btn text-xs" onclick={resetPricing} disabled={saving}>Reset</button>
        {/if}
        <button
          class="btn text-xs"
          onclick={savePricing}
          disabled={!pricingDirty || saving}
        >
          {saving ? "Saving…" : "Save & re-cost"}
        </button>
      </div>
    </div>
    <div class="text-xs text-muted">
      Saving re-costs every cached message with the new prices.
    </div>
    {#if loadingPricing}
      <div class="text-sm text-muted">Loading…</div>
    {:else}
      <div class="grid grid-cols-5 gap-2 text-xs text-muted uppercase tracking-wide">
        <div>Family</div>
        <div class="text-right">Input</div>
        <div class="text-right">Output</div>
        <div class="text-right">Cache W</div>
        <div class="text-right">Cache R</div>
      </div>
      {#each pricingDraft as p, i}
        <div class="grid grid-cols-5 gap-2 items-center text-sm border-t border-border py-2">
          <div class="truncate" title={p.model}>{p.model}</div>
          <input
            type="number"
            step="0.01"
            min="0"
            class="bg-panel2 border border-border rounded px-2 py-1 text-right num text-ink"
            bind:value={pricingDraft[i].input}
          />
          <input
            type="number"
            step="0.01"
            min="0"
            class="bg-panel2 border border-border rounded px-2 py-1 text-right num text-ink"
            bind:value={pricingDraft[i].output}
          />
          <input
            type="number"
            step="0.01"
            min="0"
            class="bg-panel2 border border-border rounded px-2 py-1 text-right num text-ink"
            bind:value={pricingDraft[i].cache_write}
          />
          <input
            type="number"
            step="0.01"
            min="0"
            class="bg-panel2 border border-border rounded px-2 py-1 text-right num text-ink"
            bind:value={pricingDraft[i].cache_read}
          />
        </div>
      {/each}
    {/if}
  </div>

  <!-- M5 placeholder -->
  <div class="card space-y-2">
    <div class="card-title">Background mode (M5)</div>
    <div class="text-sm text-muted">
      Menu-bar agent + advisory notifications. Not yet implemented.
    </div>
  </div>
</div>
