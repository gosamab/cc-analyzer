<script lang="ts">
  import { ipc, type Summary, type Utilization, type DayRow, type Recommendation, type BlockUsage, type HealthSignal } from "../ipc";
  import { ui } from "../store.svelte";
  import { fmtUsd, fmtUsdShort, fmtInt, fmtTok, fmtPct, fmtDec, fmtDuration, isoRange, today, daysAgo, shortProject, impactOfLimit } from "../format";
  import RangePicker from "../components/RangePicker.svelte";
  import Sparkline from "../components/Sparkline.svelte";
  import Donut from "../components/Donut.svelte";
  import Hint from "../components/Hint.svelte";
  import DatePicker from "../components/DatePicker.svelte";
  import { donutColor } from "../components/palette";

  let summary = $state<Summary | null>(null);
  let util = $state<Utilization | null>(null);
  let series = $state<DayRow[]>([]);
  let recs = $state<Recommendation[]>([]);
  let healthy = $state<HealthSignal[]>([]);
  let block = $state<BlockUsage | null>(null);
  // Per-slice loading flags so each card unblocks independently.
  let loadingSummary = $state(true);
  let loadingUtil = $state(true);
  let loadingSeries = $state(true);
  let loadingRecs = $state(true);
  let loadingBlock = $state(true);

  let selectedDay = $state(today());
  let dashTab = $state<"today" | "trends">("trends");

  let limitInput = $state("");
  let editingLimit = $state(false);
  let savingLimit = $state(false);

  // Monotonic token: ignore results from a stale range when the user switches range fast.
  let reqId = 0;
  let utilReqId = 0;

  $effect(() => {
    const r = ui.range;
    const { since, until } = isoRange(r);
    const mine = ++reqId;
    loadingSummary = loadingSeries = loadingRecs = loadingBlock = true;

    ipc.summary(since, until)
      .then((s) => { if (mine === reqId) summary = s; })
      .catch(console.error)
      .finally(() => { if (mine === reqId) loadingSummary = false; });

    ipc.dailyBreakdown(since, until)
      .then((d) => { if (mine === reqId) series = d; })
      .catch(console.error)
      .finally(() => { if (mine === reqId) loadingSeries = false; });

    ipc.blockUsage()
      .then((b) => {
        if (mine === reqId) {
          block = b;
          if (!editingLimit) limitInput = b.limit_tokens ? String(b.limit_tokens) : "";
        }
      })
      .catch(console.error)
      .finally(() => { if (mine === reqId) loadingBlock = false; });

    // Recs are the heaviest — fire last so the lighter queries grab the SQLite lock first.
    ipc.recommendations(since, until)
      .then((rr) => { if (mine === reqId) recs = rr; })
      .catch(console.error)
      .finally(() => { if (mine === reqId) loadingRecs = false; });

    ipc.healthSignals(since, until)
      .then((h) => { if (mine === reqId) healthy = h; })
      .catch(console.error);
  });

  // Day-scoped util load — independent so it doesn't refire on range changes.
  $effect(() => {
    const day = selectedDay;
    const mine = ++utilReqId;
    loadingUtil = true;
    ipc.utilization(day)
      .then((u) => { if (mine === utilReqId) util = u; })
      .catch(console.error)
      .finally(() => { if (mine === utilReqId) loadingUtil = false; });
  });

  // Live tick so seconds_remaining counts down without re-fetching.
  let tickNow = $state(Date.now());
  $effect(() => {
    const id = setInterval(() => { tickNow = Date.now(); }, 1000);
    return () => clearInterval(id);
  });

  const totalTokens = $derived(
    summary
      ? summary.input_tok + summary.output_tok + summary.cache_w_tok + summary.cache_r_tok
      : 0
  );

  const cacheReuse = $derived.by(() => {
    if (!summary) return 0;
    const tot = summary.cache_w_tok + summary.cache_r_tok;
    return tot > 0 ? (summary.cache_r_tok / tot) * 100 : 0;
  });

  const tokenSeries = $derived(series.map((d) => d.tokens_total));
  const costSeries = $derived(series.map((d) => d.cost_usd));
  const turnSeries = $derived(series.map((d) => d.msgs));

  const avgContext = $derived(
    summary && summary.msgs > 0
      ? (summary.input_tok + summary.cache_w_tok + summary.cache_r_tok) / summary.msgs
      : 0
  );
  const avgOutput = $derived(
    summary && summary.msgs > 0 ? summary.output_tok / summary.msgs : 0
  );

  const isToday = $derived(selectedDay === today());
  const dayLabel = $derived(
    selectedDay === today() ? "Today"
    : selectedDay === daysAgo(1) ? "Yesterday"
    : selectedDay
  );


  const ganttRows = $derived.by(() => {
    if (!util?.blocks.length) return [];
    const byProj: Record<string, typeof util.blocks> = {};
    for (const b of util.blocks) {
      (byProj[b.top_project] ||= []).push(b);
    }
    return Object.entries(byProj)
      .map(([project, blocks]) => ({
        project,
        blocks,
        total: blocks.reduce((a, b) => a + b.cost_usd, 0),
      }))
      .sort((a, b) => b.total - a.total);
  });

  function minOfDay(ts: string) {
    const h = parseInt(ts.slice(11, 13)) || 0;
    const m = parseInt(ts.slice(14, 16)) || 0;
    return h * 60 + m;
  }

  const sevColor = (s: string) =>
    s === "HIGH" ? "pill-err" : s === "MED" ? "pill-warn" : "pill-ok";

  type DonutSlice<T> = { label: string; value: number; raw: T | null };
  const projTotal = $derived(
    summary ? summary.by_project.reduce((a, p) => a + p.tokens_total, 0) : 0
  );
  const projSlices = $derived.by<DonutSlice<NonNullable<Summary["by_project"][number]>>[]>(() => {
    if (!summary) return [];
    type S = DonutSlice<NonNullable<Summary["by_project"][number]>>;
    const top = summary.by_project.slice(0, 7);
    const other = summary.by_project.slice(7).reduce((a, p) => a + p.tokens_total, 0);
    const out: S[] = top.map((p) => ({ label: shortProject(p.project), value: p.tokens_total, raw: p }));
    if (other > 0) out.push({ label: "Other", value: other, raw: null });
    return out;
  });
  const modelSlices = $derived.by<DonutSlice<NonNullable<Summary["by_model"][number]>>[]>(() => {
    if (!summary) return [];
    return [...summary.by_model]
      .sort((a, b) => b.tokens_total - a.tokens_total)
      .map((m) => ({ label: m.model.replace(/^claude-/, ""), value: m.tokens_total, raw: m }));
  });
  const modelTotal = $derived(modelSlices.reduce((a, s) => a + s.value, 0));

  const blockPct = $derived.by(() => {
    if (!block || block.limit_tokens <= 0) return 0;
    return Math.min(100, (block.tokens / block.limit_tokens) * 100);
  });
  const blockSevClass = $derived(
    blockPct >= 90 ? "bg-err" : blockPct >= 75 ? "bg-warn" : "bg-accent"
  );
  const blockSevPill = $derived(
    blockPct >= 90 ? "pill-err" : blockPct >= 75 ? "pill-warn" : "pill-ok"
  );
  const liveSecondsRemaining = $derived.by(() => {
    if (!block || !block.active || !block.block_end) return 0;
    const endMs = Date.parse(block.block_end);
    return Math.max(0, Math.floor((endMs - tickNow) / 1000));
  });
  function fmtCountdown(sec: number) {
    const h = Math.floor(sec / 3600);
    const m = Math.floor((sec % 3600) / 60);
    const s = sec % 60;
    return `${h}h ${String(m).padStart(2, "0")}m ${String(s).padStart(2, "0")}s`;
  }
  function parseLimit(s: string): number | null {
    const v = s.trim().toLowerCase();
    if (!v) return 0;
    const m = v.match(/^([\d.]+)\s*([kmb])?$/);
    if (!m) return null;
    const n = parseFloat(m[1]);
    if (!isFinite(n)) return null;
    const mult = m[2] === "k" ? 1_000 : m[2] === "m" ? 1_000_000 : m[2] === "b" ? 1_000_000_000 : 1;
    return Math.round(n * mult);
  }
  async function saveLimit() {
    const parsed = parseLimit(limitInput);
    if (parsed === null) return;
    savingLimit = true;
    try {
      await ipc.setSetting("block_limit_tokens", String(parsed));
      block = await ipc.blockUsage();
      editingLimit = false;
    } catch (e) {
      console.error(e);
    } finally {
      savingLimit = false;
    }
  }

  const dayShortcuts = $derived([
    { label: "Today", value: today() },
    { label: "Yesterday", value: daysAgo(1) },
    { label: "−2d", value: daysAgo(2) },
    { label: "−7d", value: daysAgo(7) },
  ]);
  const isCustomDay = $derived(!dayShortcuts.some((s) => s.value === selectedDay));
</script>

<div class="p-6 space-y-5">
  <div class="flex items-center justify-between">
    <div class="flex items-center gap-4">
      <h1 class="text-lg font-semibold">Dashboard</h1>
      <div class="inline-flex bg-panel2 border border-border rounded-md overflow-hidden">
        <button
          class="px-3 py-1 text-xs {dashTab === 'trends' ? 'bg-accent/90 text-bg' : 'text-muted hover:text-ink'}"
          onclick={() => (dashTab = "trends")}
        >
          Trends
        </button>
        <button
          class="px-3 py-1 text-xs {dashTab === 'today' ? 'bg-accent/90 text-bg' : 'text-muted hover:text-ink'}"
          onclick={() => (dashTab = "today")}
        >
          Today
        </button>
      </div>
    </div>
    {#if dashTab === "trends"}
      <RangePicker />
    {/if}
  </div>

  {#if !summary && loadingSummary}
    <div class="text-muted text-sm">Loading…</div>
  {:else if summary}

    {#if dashTab === "today"}
    <div class="card">
      <div class="flex items-baseline justify-between mb-2">
        <div class="card-title !mb-0">Current 5h block</div>
        <div class="text-xs text-muted flex items-center">
          {#if block?.active}
            <span class="pill {blockSevPill} mr-2">{fmtPct(blockPct, 1)}</span>
            <Hint
              text={`A 5h block starts at your first message after a >5h gap and expires 5h later. This block started at ${block.block_start?.slice(11,16)} so it ends at ${block.block_end?.slice(11,16)}.`}
              side="right"
            >
              {#snippet children()}
                <span class="cursor-help underline decoration-dotted decoration-muted/40 underline-offset-2">ends in</span>
              {/snippet}
            </Hint>
            <span class="num ml-1">{fmtCountdown(liveSecondsRemaining)}</span>
          {:else if block && block.block_start}
            <span class="text-muted">last block ended {block.block_end?.slice(11,16)}</span>
          {:else}
            <span>no recent activity</span>
          {/if}
        </div>
      </div>

      <div>
        <div class="flex items-baseline gap-3 mb-1">
          <span class="text-2xl num">{fmtTok(block?.tokens ?? 0)}</span>
          <span class="text-xs text-muted num">
            of {block && block.limit_tokens > 0 ? fmtTok(block.limit_tokens) : "—"}
            {#if block?.limit_source === "auto"}
              <span class="pill pill-ok ml-1" title="Auto-detected from your historical 5h-block max over the last 30 days">auto</span>
            {:else if block?.limit_source === "manual"}
              <span class="pill ml-1" title="Manual override (click 'override' to change)">manual</span>
            {/if}
            · {fmtInt(block?.msgs ?? 0)} turns · {fmtUsd(block?.cost_usd ?? 0)}
          </span>
        </div>
        {#if block && block.limit_tokens > 0}
          <div class="relative h-3 rounded-sm overflow-hidden bg-panel2 border border-border">
            <div class="{blockSevClass} h-full transition-all" style="width: {blockPct}%"></div>
            {#if block.limit_source === "auto" && block.historical_p90 > 0 && block.historical_p90 < block.limit_tokens}
              {@const p90Pct = (block.historical_p90 / block.limit_tokens) * 100}
              <div class="absolute top-0 bottom-0 w-px bg-muted/60" style="left: {p90Pct}%" title="p90 of past blocks: {fmtTok(block.historical_p90)}"></div>
            {/if}
          </div>
          <div class="flex justify-between text-[10px] text-muted num mt-1">
            <span>{block.block_start?.slice(11,16) ?? ""}</span>
            <span>
              {fmtPct(blockPct, 1)} used
              {#if block.limit_source === "auto" && block.historical_blocks > 0}
                · ceiling from {block.historical_blocks} past blocks
              {/if}
            </span>
            <span>{block.block_end?.slice(11,16) ?? ""}</span>
          </div>
        {:else}
          <div class="text-xs text-muted">Not enough history yet — once you have a few 5h blocks logged, an auto-ceiling will appear here.</div>
        {/if}

        <div class="mt-3 flex items-center gap-2 text-xs">
          {#if editingLimit}
            <input
              type="text"
              bind:value={limitInput}
              placeholder="e.g. 500M (blank = auto)"
              class="w-40 px-2 py-1 bg-panel2 border border-border rounded num"
              onkeydown={(e) => e.key === "Enter" && saveLimit()}
            />
            <button class="btn btn-accent text-xs" onclick={saveLimit} disabled={savingLimit}>
              {savingLimit ? "…" : "Save"}
            </button>
            <button
              class="btn text-xs"
              onclick={() => { editingLimit = false; limitInput = block?.limit_tokens ? String(block.limit_tokens) : ""; }}
            >
              Cancel
            </button>
            {#if block?.limit_source === "manual"}
              <button
                class="text-muted hover:text-ink underline underline-offset-2"
                onclick={async () => { await ipc.setSetting("block_limit_tokens", "0"); block = await ipc.blockUsage(); editingLimit = false; }}
              >
                Clear override (use auto)
              </button>
            {/if}
          {:else}
            <button
              class="text-muted hover:text-ink underline underline-offset-2"
              onclick={() => { editingLimit = true; limitInput = block?.limit_source === "manual" && block.limit_tokens ? String(block.limit_tokens) : ""; }}
            >
              {block?.limit_source === "manual" ? "Edit override" : "Override limit"}
            </button>
          {/if}
        </div>
      </div>
    </div>
    {/if}

    {#if dashTab === "trends"}
    <div class="grid grid-cols-4 gap-4">
      <div class="card">
        <div class="card-title">Tokens · {ui.range}d</div>
        <div class="text-2xl num">{fmtTok(totalTokens)}</div>
        <div class="text-xs text-muted num">{fmtInt(summary.msgs)} turns</div>
        <div class="mt-2"><Sparkline values={tokenSeries} /></div>
      </div>
      <div class="card">
        <div class="card-title">Cost · {ui.range}d</div>
        <div class="text-2xl num">{fmtUsd(summary.total_cost_usd)}</div>
        <div class="text-xs text-muted num">avg {fmtUsd(summary.total_cost_usd / Math.max(series.length, 1))}/day</div>
        <div class="mt-2"><Sparkline values={costSeries} /></div>
      </div>
      <div class="card flex flex-col">
        <div class="card-title">Cache reuse</div>
        <div class="text-2xl num">{fmtPct(cacheReuse)}</div>
        <div class="text-xs text-muted num">
          {fmtTok(summary.cache_r_tok)} read · {fmtTok(summary.cache_w_tok)} written
        </div>
        <div class="mt-2 flex items-center gap-2">
          {#if cacheReuse >= 90}<span class="pill pill-ok">healthy</span>
          {:else if cacheReuse >= 50}<span class="pill pill-warn">ok</span>
          {:else}<span class="pill pill-err">low</span>{/if}
          <div class="flex-1 h-2 rounded-sm overflow-hidden bg-panel2 border border-border flex">
            <div class="bg-accent/60 h-full" style="width: {cacheReuse}%" title="reads"></div>
            <div class="bg-warn/50 h-full" style="width: {100 - cacheReuse}%" title="writes"></div>
          </div>
        </div>
      </div>
      <div class="card">
        <div class="card-title">Turns · {ui.range}d</div>
        <div class="text-2xl num">{fmtInt(summary.msgs)}</div>
        <div class="text-xs text-muted num">
          avg ctx {fmtTok(avgContext)} · out {fmtTok(avgOutput)}
        </div>
        <div class="mt-2"><Sparkline values={turnSeries} /></div>
      </div>
    </div>

    <div class="grid grid-cols-2 gap-4 items-start">
      <div class="card">
        <div class="grid grid-cols-2 gap-4">
          <div>
            <div class="card-title">By project · tokens</div>
            <div class="flex flex-col items-center gap-3">
              <Donut
                slices={projSlices}
                size={140}
                thickness={20}
                centerLabel={fmtTok(projTotal)}
                centerSub="tokens"
              />
              <div class="w-full min-w-0 space-y-1">
                {#each projSlices as s, i}
                  {@const pct = projTotal > 0 ? (s.value / projTotal) * 100 : 0}
                  <button
                    class="w-full flex items-center gap-2 text-xs row-hover px-1 py-0.5 rounded text-left"
                    onclick={() => s.raw && ui.drillProject(s.raw.project)}
                    disabled={!s.raw}
                    title={s.raw?.project ?? s.label}
                  >
                    <span class="w-2 h-2 rounded-sm shrink-0" style="background: {donutColor(i)}"></span>
                    <span class="flex-1 min-w-0 truncate text-muted">{s.label}</span>
                    <span class="num shrink-0">{fmtTok(s.value)}</span>
                    <span class="w-10 text-right num text-muted shrink-0">{fmtPct(pct, 0)}</span>
                  </button>
                {/each}
              </div>
            </div>
          </div>

          <div>
            <div class="card-title">By model · tokens</div>
            <div class="flex flex-col items-center gap-3">
              <Donut
                slices={modelSlices}
                size={140}
                thickness={20}
                centerLabel={fmtTok(modelTotal)}
                centerSub="tokens"
              />
              <div class="w-full min-w-0 space-y-1">
                {#each modelSlices as s, i}
                  {@const pct = modelTotal > 0 ? (s.value / modelTotal) * 100 : 0}
                  <div class="flex items-center gap-2 text-xs px-1 py-0.5" title={s.raw?.model ?? s.label}>
                    <span class="w-2 h-2 rounded-sm shrink-0" style="background: {donutColor(i)}"></span>
                    <span class="flex-1 min-w-0 truncate text-muted">{s.label}</span>
                    <span class="num shrink-0">{fmtTok(s.value)}</span>
                    <span class="w-10 text-right num text-muted shrink-0">{fmtPct(pct, 0)}</span>
                  </div>
                {/each}
              </div>
            </div>
          </div>
        </div>
      </div>

      <div class="space-y-4">
        <div class="card">
          <div class="flex items-center justify-between mb-2">
            <div class="card-title !mb-0">Top recommendations</div>
            <button class="text-xs text-muted hover:text-ink" onclick={() => ui.open("insights")}>See all →</button>
          </div>
          {#if recs.length}
            <div class="space-y-1">
              {#each recs.slice(0, 6) as r}
                <div class="flex items-center gap-3 text-sm py-1">
                  <span class="pill {sevColor(r.severity)} shrink-0">{r.severity}</span>
                  <div class="min-w-0 flex-1 truncate font-medium">{r.title}</div>
                  <div class="text-right shrink-0 text-xs num text-muted">
                    {#if r.estimated_savings_tokens > 0}
                      ~{fmtTok(r.estimated_savings_tokens)} tok
                    {/if}
                    {#if r.estimated_savings_usd > 0}
                      · ~{fmtUsd(r.estimated_savings_usd)}
                    {/if}
                  </div>
                </div>
              {/each}
            </div>
          {:else if loadingRecs}
            <div class="text-sm text-muted">Loading…</div>
          {:else}
            <div class="text-sm text-muted">No recommendations for this range.</div>
          {/if}
        </div>

        {#if healthy.length}
          <div class="card">
            <div class="card-title">What's healthy</div>
            <div class="space-y-1">
              {#each healthy as h}
                <div class="flex items-start gap-3 text-sm py-1">
                  <span class="pill pill-ok shrink-0">OK</span>
                  <div class="min-w-0 flex-1">
                    <div class="font-medium truncate">{h.title}</div>
                    <div class="text-xs text-muted truncate">{h.detail}</div>
                  </div>
                </div>
              {/each}
            </div>
          </div>
        {/if}
      </div>
    </div>

    {/if}

    {#if dashTab === "today"}
    <div class="space-y-2">
      <div class="flex items-center justify-between">
        <div class="text-xs text-muted uppercase tracking-wider">
          {dayLabel} · timeline & hourly turns
        </div>
        <div class="inline-flex bg-panel2 border border-border rounded-md overflow-hidden">
          {#each dayShortcuts as s}
            <button
              class="px-3 py-1 text-xs {selectedDay === s.value ? 'bg-accent/90 text-bg' : 'text-muted hover:text-ink'}"
              onclick={() => (selectedDay = s.value)}
            >
              {s.label}
            </button>
          {/each}
          <div class="border-l border-border flex items-center {!isCustomDay ? 'text-muted hover:text-ink' : 'bg-accent/90 text-bg'}">
            <DatePicker bind:value={selectedDay} max={today()} align="right">
              <span class="px-3 py-1 text-xs num inline-block">
                {isCustomDay ? selectedDay.slice(5) : 'Pick…'}
              </span>
            </DatePicker>
          </div>
        </div>
      </div>

      {#if loadingUtil && !util}
        <div class="card text-sm text-muted">Loading…</div>
      {:else if util && util.turns > 0}
        {@const u = util}
        <div class="grid grid-cols-2 gap-4">
          <div class="card">
            <div class="card-title">{dayLabel} · timeline</div>
            {#if ganttRows.length}
              <div class="space-y-2">
                <div class="relative h-3 text-[10px] text-muted num">
                  {#each [0, 6, 12, 18, 24] as h}
                    <span class="absolute" style="left: {(h/24)*100}%; transform: translateX(-50%)">{String(h).padStart(2,'0')}</span>
                  {/each}
                </div>
                {#each ganttRows as row}
                  <div class="flex items-center gap-2 text-xs" title={row.project}>
                    <div class="w-28 shrink-0 truncate text-muted">{shortProject(row.project)}</div>
                    <div class="flex-1 relative h-5 bg-border/40 rounded-sm overflow-hidden">
                      {#each row.blocks as b}
                        {@const startPct = (minOfDay(b.start) / 1440) * 100}
                        {@const widthPct = Math.max((b.minutes / 1440) * 100, 0.4)}
                        <div
                          class="absolute h-full bg-accent/80 hover:bg-accent rounded-sm"
                          style="left: {startPct}%; width: {widthPct}%"
                          title="{b.start.slice(11,16)}–{b.end.slice(11,16)} · {fmtInt(b.turns)} turns · {fmtUsd(b.cost_usd)}"
                        ></div>
                      {/each}
                    </div>
                    <div class="w-20 shrink-0 text-right num text-muted" title={fmtUsd(row.total)}>{fmtUsdShort(row.total)}</div>
                  </div>
                {/each}
              </div>
            {:else}
              <div class="text-sm text-muted">No activity on {dayLabel.toLowerCase()}.</div>
            {/if}
          </div>

          <div class="card">
            <div class="card-title">{dayLabel} · hourly turns</div>
            <div class="flex items-stretch gap-1 h-28">
              {#each Array.from({length: 24}, (_,h) => u.hourly.find(x => parseInt(x.hour) === h) ?? { hour: String(h).padStart(2,'0'), turns: 0 }) as h}
                {@const maxTurns = Math.max(...u.hourly.map(x => x.turns), 1)}
                <div class="flex-1 flex flex-col items-center gap-1 min-w-0">
                  <div class="flex-1 w-full bg-border/40 rounded-sm overflow-hidden flex flex-col-reverse">
                    <div
                      class="w-full bg-accent/70 hover:bg-accent transition-colors"
                      style="height: {(h.turns / maxTurns) * 100}%"
                      title="{h.hour}:00 — {fmtInt(h.turns)} turns"
                    ></div>
                  </div>
                  <div class="text-[10px] text-muted num shrink-0">{h.hour}</div>
                </div>
              {/each}
            </div>
            <div class="mt-3 grid grid-cols-3 gap-2 text-xs">
              <div>
                <Hint text="Total time spent in activity blocks. A block is a contiguous run of messages with gaps ≤ 10 minutes.">
                  {#snippet children()}
                    <span class="text-muted cursor-help underline decoration-dotted decoration-muted/40 underline-offset-2">Active</span>
                  {/snippet}
                </Hint>
                <div class="num">{fmtDuration(u.active_min)}</div>
              </div>
              <div>
                <Hint text="Turns divided by active hours.">
                  {#snippet children()}
                    <span class="text-muted cursor-help underline decoration-dotted decoration-muted/40 underline-offset-2">Turns/h</span>
                  {/snippet}
                </Hint>
                <div class="num">{fmtInt(u.turns_per_active_hour)}</div>
              </div>
              <div>
                <Hint text={`Active minutes ÷ span minutes. Span is from the first message of the day to the last. ${dayLabel}: ${fmtDuration(u.active_min)} active of ${fmtDuration(u.span_min)} span.`}>
                  {#snippet children()}
                    <span class="text-muted cursor-help underline decoration-dotted decoration-muted/40 underline-offset-2">Utilization</span>
                  {/snippet}
                </Hint>
                <div class="num">{fmtPct(u.utilization_pct)}</div>
              </div>
            </div>
          </div>
        </div>
      {:else}
        <div class="card text-sm text-muted">No activity on {dayLabel.toLowerCase()}.</div>
      {/if}
    </div>
    {/if}

  {/if}
</div>
