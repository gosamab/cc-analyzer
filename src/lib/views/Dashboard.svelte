<script lang="ts">
  import { ipc, type Summary, type Utilization, type DayRow, type Recommendation } from "../ipc";
  import { ui } from "../store.svelte";
  import { fmtUsd, fmtUsdShort, fmtInt, fmtTok, fmtPct, fmtDec, fmtDuration, isoRange, today, shortProject } from "../format";
  import RangePicker from "../components/RangePicker.svelte";
  import Bar from "../components/Bar.svelte";
  import Sparkline from "../components/Sparkline.svelte";

  let summary = $state<Summary | null>(null);
  let util = $state<Utilization | null>(null);
  let series = $state<DayRow[]>([]);
  let recs = $state<Recommendation[]>([]);
  // Per-slice loading flags so each card unblocks independently.
  let loadingSummary = $state(true);
  let loadingUtil = $state(true);
  let loadingSeries = $state(true);
  let loadingRecs = $state(true);

  // Monotonic token: ignore results from a stale range when the user switches range fast.
  let reqId = 0;

  $effect(() => {
    const r = ui.range;
    const { since, until } = isoRange(r);
    const mine = ++reqId;
    loadingSummary = loadingUtil = loadingSeries = loadingRecs = true;

    ipc.summary(since, until)
      .then((s) => { if (mine === reqId) summary = s; })
      .catch(console.error)
      .finally(() => { if (mine === reqId) loadingSummary = false; });

    ipc.utilization(today())
      .then((u) => { if (mine === reqId) util = u; })
      .catch(console.error)
      .finally(() => { if (mine === reqId) loadingUtil = false; });

    ipc.dailyBreakdown(since, until)
      .then((d) => { if (mine === reqId) series = d; })
      .catch(console.error)
      .finally(() => { if (mine === reqId) loadingSeries = false; });

    // Recs are the heaviest — fire last so the lighter queries grab the SQLite lock first.
    ipc.recommendations(since, until)
      .then((rr) => { if (mine === reqId) recs = rr; })
      .catch(console.error)
      .finally(() => { if (mine === reqId) loadingRecs = false; });
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

  // Headline string answering "where am I right now"
  const headline = $derived.by(() => {
    if (!summary || !util) return "";
    const topProj = summary.by_project[0];
    const todayTokens = util.turns > 0
      ? series[series.length - 1]?.tokens_total ?? 0
      : 0;
    if (util.turns === 0) {
      return `No activity yet today. Range total: ${fmtTok(totalTokens)} tokens, ${fmtInt(summary.msgs)} turns.`;
    }
    const dayCost = series[series.length - 1]?.cost_usd ?? 0;
    return `Today: ${fmtTok(todayTokens)} tokens · ${fmtInt(util.turns)} turns · ${fmtUsd(dayCost)}${
      topProj ? ` · top project ${shortProject(topProj.project)}` : ""
    }`;
  });

  // Today's Gantt: group blocks by top_project; place by clock-time.
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
</script>

<div class="p-6 space-y-5">
  <div class="flex items-center justify-between">
    <h1 class="text-lg font-semibold">Dashboard</h1>
    <RangePicker />
  </div>

  {#if !summary && loadingSummary}
    <div class="text-muted text-sm">Loading…</div>
  {:else if summary}
    <!-- Headline -->
    <div class="card !p-3 bg-panel2/60">
      <div class="text-sm">{headline || "…"}</div>
    </div>

    <!-- KPI strip with sparklines -->
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
          avg ctx {fmtTok(util?.avg_context ?? 0)} · out {fmtTok(util?.avg_output ?? 0)}
        </div>
        <div class="mt-2"><Sparkline values={turnSeries} /></div>
      </div>
    </div>

    <!-- By project (left) + by model (right) — token bars -->
    <div class="grid grid-cols-2 gap-4">
      <div class="card">
        <div class="card-title">By project · tokens</div>
        <div class="space-y-2">
          {#each summary.by_project.slice(0, 8) as p}
            <button class="w-full flex items-center gap-3 text-sm row-hover px-1 py-0.5 rounded" onclick={() => ui.drillProject(p.project)} title={p.project}>
              <div class="w-40 shrink-0 truncate text-left text-muted">{shortProject(p.project)}</div>
              <div class="flex-1 min-w-0"><Bar value={p.tokens_total} max={summary.by_project[0].tokens_total} /></div>
              <div class="w-20 shrink-0 text-right num">{fmtTok(p.tokens_total)}</div>
              <div class="w-20 shrink-0 text-right num text-muted" title={fmtUsd(p.cost_usd)}>{fmtUsdShort(p.cost_usd)}</div>
            </button>
          {/each}
        </div>
      </div>

      <div class="card">
        <div class="card-title">By model · tokens</div>
        <div class="space-y-2">
          {#each [...summary.by_model].sort((a,b)=>b.tokens_total-a.tokens_total) as m}
            <div class="flex items-center gap-3 text-sm">
              <div class="w-40 shrink-0 truncate text-muted">{m.model}</div>
              <div class="flex-1 min-w-0"><Bar value={m.tokens_total} max={summary.by_model.reduce((a,x)=>Math.max(a,x.tokens_total),1)} /></div>
              <div class="w-20 shrink-0 text-right num">{fmtTok(m.tokens_total)}</div>
              <div class="w-20 shrink-0 text-right num text-muted" title={fmtUsd(m.cost_usd)}>{fmtUsdShort(m.cost_usd)}</div>
            </div>
          {/each}
        </div>
      </div>
    </div>

    <!-- Today: Gantt timeline (left) + hourly heatmap (right) -->
    {#if util && util.turns > 0}
      {@const u = util}
      <div class="grid grid-cols-2 gap-4">
        <div class="card">
          <div class="card-title">Today · timeline</div>
          {#if ganttRows.length}
            <div class="space-y-2">
              <!-- Hour ruler -->
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
            <div class="text-sm text-muted">No activity yet today.</div>
          {/if}
        </div>

        <div class="card">
          <div class="card-title">Today · hourly turns</div>
          <div class="flex items-end gap-1 h-28">
            {#each Array.from({length: 24}, (_,h) => u.hourly.find(x => parseInt(x.hour) === h) ?? { hour: String(h).padStart(2,'0'), turns: 0 }) as h}
              {@const maxTurns = Math.max(...u.hourly.map(x => x.turns), 1)}
              <div class="flex-1 flex flex-col items-center gap-1">
                <div class="w-full bg-border/40 rounded-sm overflow-hidden flex flex-col-reverse" style="height: 100%">
                  <div
                    class="w-full bg-accent/70 hover:bg-accent transition-colors"
                    style="height: {(h.turns / maxTurns) * 100}%"
                    title="{h.hour}:00 — {fmtInt(h.turns)} turns"
                  ></div>
                </div>
                <div class="text-[10px] text-muted num">{h.hour}</div>
              </div>
            {/each}
          </div>
          <div class="mt-3 grid grid-cols-3 gap-2 text-xs">
            <div>
              <div class="text-muted">Active</div>
              <div class="num">{fmtDuration(u.active_min)}</div>
            </div>
            <div>
              <div class="text-muted">Turns/h</div>
              <div class="num">{fmtInt(u.turns_per_active_hour)}</div>
            </div>
            <div>
              <div class="text-muted">Utilization</div>
              <div class="num">{fmtPct(u.utilization_pct)}</div>
            </div>
          </div>
        </div>
      </div>
    {/if}

    <!-- Inline top recommendations -->
    {#if recs.length}
      <div class="card">
        <div class="flex items-center justify-between mb-2">
          <div class="card-title !mb-0">Top recommendations</div>
          <button class="text-xs text-muted hover:text-ink" onclick={() => ui.open("insights")}>See all →</button>
        </div>
        <div class="space-y-2">
          {#each recs.slice(0, 3) as r}
            <div class="flex items-start gap-3 text-sm py-1">
              <span class="pill {sevColor(r.severity)} shrink-0">{r.severity}</span>
              <div class="min-w-0 flex-1">
                <div class="font-medium">{r.title}</div>
                <div class="text-muted text-xs">{r.body}</div>
              </div>
              <div class="text-right shrink-0 text-xs num">
                {#if r.estimated_savings_tokens > 0}
                  <div>~{fmtTok(r.estimated_savings_tokens)} tokens</div>
                {/if}
                {#if r.estimated_savings_usd > 0}
                  <div class="text-muted">~{fmtUsd(r.estimated_savings_usd)}/mo</div>
                {/if}
              </div>
            </div>
          {/each}
        </div>
      </div>
    {/if}
  {/if}
</div>
