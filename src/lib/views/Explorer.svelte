<script lang="ts">
  import { ipc, type SessionRow, type SessionDetail, type TurnRow } from "../ipc";
  import { ui } from "../store.svelte";
  import { fmtUsd, fmtUsdShort, fmtInt, fmtTok, fmtDuration, isoRange, shortProject, shortSession, redactPath, redactText } from "../format";
  import RangePicker from "../components/RangePicker.svelte";

  let allSessions = $state<SessionRow[]>([]);
  let detail = $state<SessionDetail | null>(null);
  let loading = $state(true);

  let modelFilter = $state<string>("");
  let minTokens = $state<number>(0);
  let minTurns = $state<number>(0);
  let search = $state<string>("");
  type SortField = "session" | "project" | "turns" | "tokens" | "cost" | "dur";
  type SortDir = "asc" | "desc";
  let sortField = $state<SortField | null>("tokens");
  let sortDir = $state<SortDir>("desc");

  function toggleSort(field: SortField) {
    if (sortField !== field) {
      sortField = field;
      sortDir = "desc";
    } else if (sortDir === "desc") {
      sortDir = "asc";
    } else {
      sortField = null;
      sortDir = "desc";
    }
  }
  const sortArrow = (field: SortField) =>
    sortField === field ? (sortDir === "desc" ? " ↓" : " ↑") : "";

  $effect(() => {
    const r = ui.range;
    const { since, until } = isoRange(r);
    loading = true;
    ipc.sessions(since, until)
      .then((s) => {
        allSessions = s;
        if (ui.selectedSession) loadDetail(ui.selectedSession);
      })
      .finally(() => (loading = false));
  });

  // React to deep-links from other views (Insights "Open session" etc.) — load
  // the detail and scroll the matching list row into view.
  $effect(() => {
    const id = ui.selectedSession;
    if (!id) return;
    if (detail?.session_id !== id) loadDetail(id);
    // Defer scroll to next frame so the row exists in the DOM.
    queueMicrotask(() => {
      const row = document.querySelector<HTMLElement>(`[data-session-id="${id}"]`);
      row?.scrollIntoView({ block: "nearest", behavior: "smooth" });
    });
  });

  const projects = $derived([...new Set(allSessions.map((s) => s.project))].sort());
  const models = $derived([...new Set(allSessions.map((s) => s.model))].sort());

  const filtered = $derived.by(() => {
    let s = allSessions;
    if (ui.projectFilter) s = s.filter((x) => x.project === ui.projectFilter);
    if (modelFilter) s = s.filter((x) => x.model === modelFilter);
    if (minTokens > 0) s = s.filter((x) => x.tokens_total >= minTokens);
    if (minTurns > 0) s = s.filter((x) => x.msgs >= minTurns);
    if (search.trim()) {
      const q = search.toLowerCase();
      s = s.filter(
        (x) =>
          x.session_id.toLowerCase().includes(q) ||
          x.project.toLowerCase().includes(q) ||
          (x.title?.toLowerCase().includes(q) ?? false)
      );
    }
    if (!sortField) return s;
    const sign = sortDir === "asc" ? 1 : -1;
    const dur = (x: SessionRow) => new Date(x.end_ts).getTime() - new Date(x.start_ts).getTime();
    return [...s].sort((a, b) => {
      switch (sortField) {
        case "session": return sign * a.session_id.localeCompare(b.session_id);
        case "project": return sign * a.project.localeCompare(b.project);
        case "turns":   return sign * (a.msgs - b.msgs);
        case "tokens":  return sign * (a.tokens_total - b.tokens_total);
        case "cost":    return sign * (a.cost_usd - b.cost_usd);
        case "dur":     return sign * (dur(a) - dur(b));
      }
      return 0;
    });
  });

  function loadDetail(id: string) {
    ui.selectedSession = id;
    ipc.sessionDetail(id).then((d) => (detail = d));
  }

  function clearAll() {
    ui.projectFilter = null;
    modelFilter = "";
    minTokens = 0;
    minTurns = 0;
    search = "";
  }

  function durMin(a: string, b: string) {
    return Math.round((new Date(b).getTime() - new Date(a).getTime()) / 60000);
  }

  const detailTokens = $derived.by(() => {
    if (!detail) return 0;
    return (
      detail.input_tok + detail.output_tok + detail.cache_w_tok + detail.cache_r_tok
    );
  });

  let hoveredTurn = $state<TurnRow | null>(null);
  let hoverX = $state(0);
  let hoverY = $state(0);

  function onTurnEnter(t: TurnRow, ev: MouseEvent) {
    hoveredTurn = t;
    hoverX = ev.clientX;
    hoverY = ev.clientY;
  }
  function onTurnMove(ev: MouseEvent) {
    hoverX = ev.clientX;
    hoverY = ev.clientY;
  }
  function onTurnLeave() {
    hoveredTurn = null;
  }

  const turnTotal = (t: TurnRow) =>
    t.input_tok + t.output_tok + t.cache_w_tok + t.cache_r_tok;

  const composition = $derived.by(() => {
    if (!detail) return { pctCr: 0, pctCw: 0, pctIn: 0, pctOut: 0 };
    const total = Math.max(detailTokens, 1);
    return {
      pctCr: (detail.cache_r_tok / total) * 100,
      pctCw: (detail.cache_w_tok / total) * 100,
      pctIn: (detail.input_tok / total) * 100,
      pctOut: (detail.output_tok / total) * 100,
    };
  });

  const MAX_BARS = 240;
  const turnBuckets = $derived.by(() => {
    if (!detail) return [] as { turns: TurnRow[]; agg: TurnRow }[];
    const turns = detail.turns;
    const size = Math.max(1, Math.ceil(turns.length / MAX_BARS));
    const out: { turns: TurnRow[]; agg: TurnRow }[] = [];
    for (let i = 0; i < turns.length; i += size) {
      const chunk = turns.slice(i, i + size);
      const agg: TurnRow = {
        ts: chunk[0].ts,
        cost_usd: 0,
        input_tok: 0,
        output_tok: 0,
        cache_w_tok: 0,
        cache_r_tok: 0,
        tools: [],
      };
      const toolMap = new Map<string, number>();
      for (const t of chunk) {
        agg.cost_usd += t.cost_usd;
        agg.input_tok += t.input_tok;
        agg.output_tok += t.output_tok;
        agg.cache_w_tok += t.cache_w_tok;
        agg.cache_r_tok += t.cache_r_tok;
        for (const tt of t.tools) toolMap.set(tt.name, (toolMap.get(tt.name) ?? 0) + tt.count);
      }
      agg.tools = [...toolMap.entries()]
        .map(([name, count]) => ({ name, count }))
        .sort((a, b) => b.count - a.count);
      out.push({ turns: chunk, agg });
    }
    return out;
  });
  const turnMax = $derived(Math.max(...turnBuckets.map((b) => turnTotal(b.agg)), 1));
  const bucketSize = $derived(turnBuckets.length ? turnBuckets[0].turns.length : 1);

  const hasFilters = $derived(
    !!(ui.projectFilter || modelFilter || minTokens || minTurns || search)
  );
</script>

<div class="flex flex-col h-full">
  <div class="border-b border-border bg-panel/60 px-4 py-2 flex items-center gap-3 flex-wrap text-sm">
    <select
      class="bg-panel2 border border-border rounded px-2 py-1 text-sm min-w-[10rem]"
      bind:value={ui.projectFilter}
    >
      <option value={null}>All projects</option>
      {#each projects as p}
        <option value={p}>{shortProject(p)}</option>
      {/each}
    </select>

    <select
      class="bg-panel2 border border-border rounded px-2 py-1 text-sm min-w-[10rem]"
      bind:value={modelFilter}
    >
      <option value="">All models</option>
      {#each models as m}
        <option value={m}>{m}</option>
      {/each}
    </select>

    <label class="flex items-center gap-1 text-xs text-muted">
      Min tokens
      <input
        type="number"
        min="0"
        step="100000"
        class="w-24 bg-panel2 border border-border rounded px-2 py-1 text-sm num text-ink"
        bind:value={minTokens}
      />
    </label>

    <label class="flex items-center gap-1 text-xs text-muted">
      Min turns
      <input
        type="number"
        min="0"
        step="10"
        class="w-20 bg-panel2 border border-border rounded px-2 py-1 text-sm num text-ink"
        bind:value={minTurns}
      />
    </label>

    <input
      type="text"
      placeholder="Search session or project…"
      class="flex-1 min-w-[12rem] bg-panel2 border border-border rounded px-2 py-1 text-sm text-ink"
      bind:value={search}
    />

    {#if hasFilters}
      <button class="btn text-xs" onclick={clearAll}>Clear</button>
    {/if}

    <div class="ml-auto flex items-center gap-3">
      <div class="text-xs text-muted num">
        {filtered.length} / {allSessions.length} sessions
      </div>
      <RangePicker />
    </div>
  </div>

  <div class="grid grid-cols-2 flex-1 min-h-0">
    <section class="border-r border-border overflow-auto">
      <div class="sticky top-0 bg-panel border-b border-border px-3 py-2 text-xs text-muted flex items-center gap-3">
        <button class="w-20 shrink-0 text-left hover:text-ink" onclick={() => toggleSort("session")}>Session{sortArrow("session")}</button>
        <button class="flex-[2] min-w-0 text-left hover:text-ink" onclick={() => toggleSort("project")}>Title · Project{sortArrow("project")}</button>
        <button class="flex-1 min-w-[3rem] text-right hover:text-ink" onclick={() => toggleSort("turns")}>Turns{sortArrow("turns")}</button>
        <button class="flex-1 min-w-[4rem] text-right hover:text-ink" onclick={() => toggleSort("tokens")}>Tokens{sortArrow("tokens")}</button>
        <button class="flex-1 min-w-[4rem] text-right hover:text-ink" onclick={() => toggleSort("cost")}>Cost{sortArrow("cost")}</button>
        <button class="flex-1 min-w-[3rem] text-right hover:text-ink" onclick={() => toggleSort("dur")}>Dur{sortArrow("dur")}</button>
      </div>
      {#if loading}
        <div class="p-4 text-muted text-sm">Loading…</div>
      {:else if !filtered.length}
        <div class="p-4 text-muted text-sm">No sessions match filters.</div>
      {:else}
        {#each filtered as s}
          <button
            class="w-full px-3 py-2 text-sm flex items-center gap-3 border-b border-border row-hover {ui.selectedSession === s.session_id ? 'bg-panel2' : ''}"
            data-session-id={s.session_id}
            onclick={() => loadDetail(s.session_id)}
            title={s.title ? `${s.title}\n${s.project}` : s.project}
          >
            <div class="w-20 shrink-0 num text-muted text-left">{shortSession(s.session_id)}</div>
            <div class="flex-[2] min-w-0 text-left">
              {#if s.title}
                <div class="truncate text-ink">{s.title}</div>
                <div class="truncate text-xs text-muted">{shortProject(s.project)}</div>
              {:else}
                <div class="truncate">{shortProject(s.project)}</div>
              {/if}
            </div>
            <div class="flex-1 min-w-[3rem] text-right num">{fmtInt(s.msgs)}</div>
            <div class="flex-1 min-w-[4rem] text-right num">{fmtTok(s.tokens_total)}</div>
            <div class="flex-1 min-w-[4rem] text-right num text-muted" title={fmtUsd(s.cost_usd)}>{fmtUsdShort(s.cost_usd)}</div>
            <div class="flex-1 min-w-[3rem] text-right num text-muted">{fmtDuration(durMin(s.start_ts, s.end_ts))}</div>
          </button>
        {/each}
      {/if}
    </section>

    <section class="overflow-auto p-4 space-y-4">
      {#if !detail}
        <div class="text-muted text-sm">Select a session to inspect.</div>
      {:else}
        <div>
          <div class="flex items-center justify-between gap-3">
            <div class="min-w-0">
              <h2 class="text-base font-medium truncate" title={detail.title ?? detail.project}>
                {detail.title ?? shortProject(detail.project)}
              </h2>
              <div class="text-xs text-muted truncate">
                {#if detail.title}{shortProject(detail.project)} · {/if}<span class="num">{detail.session_id}</span>
              </div>
            </div>
            <span class="pill shrink-0 whitespace-nowrap">{detail.model.replace("claude-", "")}</span>
          </div>
          <div class="grid grid-cols-4 gap-3 mt-3">
            <div class="card">
              <div class="card-title">Tokens</div>
              <div class="text-xl num">{fmtTok(detailTokens)}</div>
            </div>
            <div class="card">
              <div class="card-title">Turns</div>
              <div class="text-xl num">{fmtInt(detail.msgs)}</div>
            </div>
            <div class="card">
              <div class="card-title">Tokens/turn</div>
              <div class="text-xl num">{fmtTok(detailTokens / Math.max(detail.msgs, 1))}</div>
            </div>
            <div class="card">
              <div class="card-title">Cost</div>
              <div class="text-xl num text-muted" title={fmtUsd(detail.cost_usd)}>{fmtUsdShort(detail.cost_usd)}</div>
            </div>
          </div>
        </div>

        <div class="card">
          <div class="card-title">Token composition</div>
          <div class="flex h-3 rounded overflow-hidden bg-panel2 border border-border">
            <div
              class="bg-accent/30"
              style="width: {composition.pctCr}%"
              title="cache reads: {fmtTok(detail.cache_r_tok)} ({composition.pctCr.toFixed(0)}%)"
            ></div>
            <div
              class="bg-warn/60"
              style="width: {composition.pctCw}%"
              title="cache writes: {fmtTok(detail.cache_w_tok)} ({composition.pctCw.toFixed(0)}%)"
            ></div>
            <div
              class="bg-muted/40"
              style="width: {composition.pctIn}%"
              title="input: {fmtTok(detail.input_tok)} ({composition.pctIn.toFixed(0)}%)"
            ></div>
            <div
              class="bg-accent"
              style="width: {composition.pctOut}%"
              title="output: {fmtTok(detail.output_tok)} ({composition.pctOut.toFixed(0)}%)"
            ></div>
          </div>
          <div class="flex flex-wrap gap-x-4 gap-y-1 mt-2 text-xs text-muted">
            <span><span class="inline-block w-2 h-2 bg-accent/30 mr-1 align-middle"></span>cache-read <span class="num text-ink">{fmtTok(detail.cache_r_tok)}</span> ({composition.pctCr.toFixed(0)}%)</span>
            <span><span class="inline-block w-2 h-2 bg-warn/60 mr-1 align-middle"></span>cache-write <span class="num text-ink">{fmtTok(detail.cache_w_tok)}</span> ({composition.pctCw.toFixed(0)}%)</span>
            <span><span class="inline-block w-2 h-2 bg-muted/40 mr-1 align-middle"></span>input <span class="num text-ink">{fmtTok(detail.input_tok)}</span> ({composition.pctIn.toFixed(0)}%)</span>
            <span><span class="inline-block w-2 h-2 bg-accent mr-1 align-middle"></span>output <span class="num text-ink">{fmtTok(detail.output_tok)}</span> ({composition.pctOut.toFixed(0)}%)</span>
          </div>
        </div>

        <div class="card">
          <div class="card-title flex items-center justify-between">
            <span>Tokens per turn (hover for tool calls)</span>
            <span class="text-muted normal-case tracking-normal text-[10px]">
              {detail.turns.length} turns{bucketSize > 1 ? ` · ${bucketSize}/bar` : ""}
            </span>
          </div>
          <div
            role="img"
            aria-label="Per-turn token composition"
            class="flex items-end h-24 w-full overflow-hidden"
            onmouseleave={onTurnLeave}
          >
            {#each turnBuckets as b}
              {@const tot = turnTotal(b.agg)}
              <div
                role="presentation"
                class="flex-1 min-w-0 flex flex-col-reverse hover:opacity-80 cursor-default"
                style="height: {(tot / turnMax) * 100}%;"
                onmouseenter={(e) => onTurnEnter(b.agg, e)}
                onmousemove={onTurnMove}
              >
                <div class="bg-accent/30" style="flex: {b.agg.cache_r_tok}"></div>
                <div class="bg-warn/60" style="flex: {b.agg.cache_w_tok}"></div>
                <div class="bg-muted/40" style="flex: {b.agg.input_tok}"></div>
                <div class="bg-accent" style="flex: {b.agg.output_tok}"></div>
              </div>
            {/each}
          </div>
        </div>

        <div class="grid grid-cols-2 gap-3">
          <div class="card">
            <div class="card-title">Top tools</div>
            <div class="space-y-1 text-sm">
              {#each Object.entries(detail.tool_counts).sort((a,b)=>b[1]-a[1]) as [name, n]}
                <div class="flex justify-between">
                  <span class="text-muted">{name}</span>
                  <span class="num">{fmtInt(n)}</span>
                </div>
              {/each}
            </div>
          </div>
          <div class="card">
            <div class="card-title">Most-read files</div>
            <div class="space-y-1 text-sm">
              {#each detail.top_files.slice(0, 10) as f}
                <div class="flex justify-between gap-2">
                  <span class="text-muted truncate" title={redactText(f.file_path)}>{redactPath(f.file_path.split("/").pop() ?? f.file_path)}</span>
                  <span class="num">{f.count}</span>
                </div>
              {/each}
            </div>
          </div>
        </div>

        {#if detail.skills_used.length || detail.mcps_used.length || detail.slash_commands.length}
          <div class="grid grid-cols-3 gap-3">
            <div class="card">
              <div class="card-title">Skills used</div>
              {#if detail.skills_used.length}
                <div class="space-y-1 text-sm">
                  {#each detail.skills_used.slice(0, 10) as s}
                    <div class="flex justify-between gap-2">
                      <span class="text-muted truncate font-mono text-xs" title={s.name}>{s.name}</span>
                      <span class="num">{fmtInt(s.count)}</span>
                    </div>
                  {/each}
                </div>
              {:else}
                <div class="text-xs text-muted">None</div>
              {/if}
            </div>
            <div class="card">
              <div class="card-title">MCPs used</div>
              {#if detail.mcps_used.length}
                <div class="space-y-1 text-sm">
                  {#each detail.mcps_used.slice(0, 10) as m}
                    <div class="flex justify-between gap-2">
                      <span class="text-muted truncate font-mono text-xs" title={m.name}>
                        {m.name.startsWith("mcp__") ? m.name.slice(5) : m.name}
                      </span>
                      <span class="num">{fmtInt(m.count)}</span>
                    </div>
                  {/each}
                </div>
              {:else}
                <div class="text-xs text-muted">None</div>
              {/if}
            </div>
            <div class="card">
              <div class="card-title">Slash commands</div>
              {#if detail.slash_commands.length}
                <div class="space-y-1 text-sm">
                  {#each detail.slash_commands.slice(0, 10) as c}
                    <div class="flex justify-between gap-2">
                      <span class="text-muted truncate font-mono text-xs" title={c.name}>{c.name}</span>
                      <span class="num">{fmtInt(c.count)}</span>
                    </div>
                  {/each}
                </div>
              {:else}
                <div class="text-xs text-muted">None</div>
              {/if}
            </div>
          </div>
        {/if}
      {/if}
    </section>
  </div>

  {#if hoveredTurn}
    <div
      class="fixed z-50 pointer-events-none bg-panel border border-border rounded-md shadow-lg px-3 py-2 text-xs space-y-1 max-w-xs"
      style="left: {Math.min(hoverX + 12, window.innerWidth - 280)}px; top: {Math.min(hoverY + 12, window.innerHeight - 200)}px;"
    >
      <div class="num text-muted">{hoveredTurn.ts.replace("T", " ").slice(0, 19)}</div>
      <div class="flex justify-between gap-4">
        <span class="text-muted">tokens</span>
        <span class="num">{fmtTok(turnTotal(hoveredTurn))}</span>
      </div>
      <div class="flex justify-between gap-4">
        <span class="text-muted">cache r/w</span>
        <span class="num">{fmtTok(hoveredTurn.cache_r_tok)} / {fmtTok(hoveredTurn.cache_w_tok)}</span>
      </div>
      <div class="flex justify-between gap-4">
        <span class="text-muted">in / out</span>
        <span class="num">{fmtTok(hoveredTurn.input_tok)} / {fmtTok(hoveredTurn.output_tok)}</span>
      </div>
      <div class="flex justify-between gap-4">
        <span class="text-muted">cost</span>
        <span class="num text-muted">{fmtUsd(hoveredTurn.cost_usd)}</span>
      </div>
      {#if hoveredTurn.tools.length}
        <div class="border-t border-border pt-1 mt-1">
          <div class="text-muted mb-0.5">tools</div>
          {#each hoveredTurn.tools as tool}
            <div class="flex justify-between gap-4">
              <span>{tool.name}</span>
              <span class="num text-muted">×{tool.count}</span>
            </div>
          {/each}
        </div>
      {:else}
        <div class="text-muted italic border-t border-border pt-1 mt-1">no tool calls</div>
      {/if}
    </div>
  {/if}
</div>
