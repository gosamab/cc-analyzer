<script lang="ts">
  import { ipc, type CommandRow, type ToolUsageRow, type BlockUsage, type CommandCategory } from "../ipc";
  import { fmtUsd, fmtTok, fmtInt, fmtPct, isoRange, impactOfLimit, redactText } from "../format";
  import { ui } from "../store.svelte";
  import RangePicker from "../components/RangePicker.svelte";

  let tools = $state<ToolUsageRow[]>([]);
  let commands = $state<CommandRow[]>([]);
  let block = $state<BlockUsage | null>(null);
  let loadingTools = $state(true);
  let loadingCmds = $state(true);
  let tab = $state<"tools" | "bash">("tools");
  type CmdFilter = "all" | CommandCategory;
  const cmdFilters: CmdFilter[] = [
    "all", "git", "run", "install", "search", "fs", "inspect", "script", "text", "net", "other",
  ];
  let cmdFilter = $state<CmdFilter>("all");
  let expanded = $state<Set<string>>(new Set());

  function toggle(key: string) {
    const next = new Set(expanded);
    if (next.has(key)) next.delete(key); else next.add(key);
    expanded = next;
  }

  let reqId = 0;
  $effect(() => {
    const r = ui.range;
    const { since, until } = isoRange(r);
    const mine = ++reqId;
    loadingTools = loadingCmds = true;

    ipc.toolUsage(since, until)
      .then((x) => { if (mine === reqId) tools = x; })
      .catch(console.error)
      .finally(() => { if (mine === reqId) loadingTools = false; });

    ipc.topCommands(since, until, 50)
      .then((x) => { if (mine === reqId) commands = x; })
      .catch(console.error)
      .finally(() => { if (mine === reqId) loadingCmds = false; });

    ipc.blockUsage()
      .then((b) => { if (mine === reqId) block = b; })
      .catch(console.error);
  });

  // Reserve color for the hand-typeable categories — the rest are informational.
  const catColor = (c: string) =>
    c === "git" ? "text-warn"
    : c === "install" ? "text-ok"
    : c === "run" ? "text-err"
    : "text-muted";

  const blockPct = (tokens: number) =>
    block?.limit_tokens ? (impactOfLimit(tokens, block.limit_tokens, true) ?? "—") : "—";

  let toolTotals = $derived({
    count: tools.reduce((s, t) => s + t.count, 0),
    tokens: tools.reduce((s, t) => s + t.tokens, 0),
    cost: tools.reduce((s, t) => s + t.cost_usd, 0),
  });
  let maxToolTokens = $derived(tools.reduce((m, t) => Math.max(m, t.tokens), 0));

  let filteredCmds = $derived(
    cmdFilter === "all" ? commands : commands.filter((c) => c.category === cmdFilter),
  );
  let cmdTotals = $derived({
    count: filteredCmds.reduce((s, c) => s + c.count, 0),
    tokens: filteredCmds.reduce((s, c) => s + c.tokens, 0),
    cost: filteredCmds.reduce((s, c) => s + c.cost_usd, 0),
  });
  let maxCmdTokens = $derived(filteredCmds.reduce((m, c) => Math.max(m, c.tokens), 0));
</script>

<div class="p-6 space-y-4">
  <div class="flex items-center justify-between">
    <div class="flex items-center gap-6">
      <h1 class="text-lg font-semibold">Commands</h1>
      <div class="flex gap-1 text-sm">
        <button
          class="px-3 py-1 rounded {tab === 'tools' ? 'bg-panel2 text-ink' : 'text-muted hover:text-ink'}"
          onclick={() => (tab = "tools")}
        >Tool use</button>
        <button
          class="px-3 py-1 rounded {tab === 'bash' ? 'bg-panel2 text-ink' : 'text-muted hover:text-ink'}"
          onclick={() => (tab = "bash")}
        >Bash commands</button>
      </div>
    </div>
    <RangePicker />
  </div>

  {#snippet statStrip(totals: { count: number; tokens: number; cost: number }, loading: boolean)}
    {#if loading}
      <div class="text-sm text-muted">Loading…</div>
    {:else}
      <div class="flex items-stretch gap-0 divide-x divide-border/60">
        <div class="flex flex-col justify-center pr-5">
          <div class="text-[10px] uppercase tracking-wider text-muted">Calls</div>
          <div class="text-lg num leading-tight">{fmtInt(totals.count)}</div>
        </div>
        <div class="flex flex-col justify-center px-5">
          <div class="text-[10px] uppercase tracking-wider text-muted">Tokens</div>
          <div class="text-lg num leading-tight">{fmtTok(totals.tokens)}</div>
        </div>
        <div class="flex flex-col justify-center px-5">
          <div class="text-[10px] uppercase tracking-wider text-muted">Cost</div>
          <div class="text-lg num leading-tight">{fmtUsd(totals.cost)}</div>
        </div>
        {#if block?.limit_tokens && impactOfLimit(totals.tokens, block.limit_tokens)}
          <div class="flex flex-col justify-center pl-5">
            <div class="text-[10px] uppercase tracking-wider text-muted">% of 5h block</div>
            <div class="text-lg num leading-tight">{impactOfLimit(totals.tokens, block.limit_tokens, true)}</div>
          </div>
        {/if}
      </div>
    {/if}
  {/snippet}

  {#if tab === "tools"}
    <div class="card !p-0 overflow-hidden">
      <div class="px-5 py-4 border-b border-border">
        {@render statStrip(toolTotals, loadingTools)}
      </div>

      {#if !loadingTools && !tools.length}
        <div class="p-6 text-muted text-sm">No tool calls in this range.</div>
      {:else if !loadingTools}
        <table class="w-full text-sm table-fixed">
          <thead>
            <tr class="text-xs uppercase tracking-wider text-muted">
              <th class="text-left font-normal px-5 py-2 w-[26%]">Tool</th>
              <th class="text-right font-normal px-3 py-2 w-[10%]">Calls</th>
              <th class="text-right font-normal px-3 py-2 w-[10%]">Turns</th>
              <th class="text-right font-normal px-3 py-2 w-[12%]">Tokens</th>
              <th class="text-left font-normal px-3 py-2">Share</th>
              <th class="text-right font-normal px-3 py-2 w-[14%]">Cost</th>
              <th class="text-right font-normal pl-3 pr-5 py-2 w-[10%]">% block</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-border/40">
            {#each tools as t}
              <tr class="hover:bg-panel2/60">
                <td class="px-5 py-2.5 truncate text-ink" title={t.tool}>{t.tool}</td>
                <td class="px-3 py-2.5 text-right num">{fmtInt(t.count)}</td>
                <td class="px-3 py-2.5 text-right num text-muted">{fmtInt(t.turns)}</td>
                <td class="px-3 py-2.5 text-right num">{fmtTok(t.tokens)}</td>
                <td class="px-3 py-2.5">
                  <div class="flex items-center gap-2">
                    <div class="bar flex-1">
                      <div class="fill" style="width: {maxToolTokens > 0 ? (t.tokens / maxToolTokens) * 100 : 0}%"></div>
                    </div>
                    <div class="num text-xs text-muted w-9 text-right shrink-0">
                      {toolTotals.tokens > 0 ? fmtPct((t.tokens / toolTotals.tokens) * 100, 0) : "—"}
                    </div>
                  </div>
                </td>
                <td class="px-3 py-2.5 text-right num text-muted">{fmtUsd(t.cost_usd)}</td>
                <td class="pl-3 pr-5 py-2.5 text-right num text-muted">{blockPct(t.tokens)}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    </div>
  {:else}
    <div class="card !p-0 overflow-hidden">
      <div class="px-5 py-4 border-b border-border flex items-center justify-between gap-4">
        <div class="min-w-0">{@render statStrip(cmdTotals, loadingCmds)}</div>
        <div class="flex flex-wrap gap-1 text-xs justify-end max-w-[60%]">
          {#each cmdFilters as f}
            <button
              class="px-2.5 py-1 rounded border {cmdFilter === f ? 'bg-panel2 text-ink border-border' : 'text-muted border-transparent hover:text-ink hover:border-border'}"
              onclick={() => (cmdFilter = f)}
            >{f}</button>
          {/each}
        </div>
      </div>

      {#if !loadingCmds && !filteredCmds.length}
        <div class="p-6 text-muted text-sm">No commands in this range.</div>
      {:else if !loadingCmds}
        <table class="w-full text-sm table-fixed">
          <thead>
            <tr class="text-xs uppercase tracking-wider text-muted">
              <th class="text-left font-normal pl-5 pr-3 py-2 w-8"></th>
              <th class="text-left font-normal pr-3 py-2 w-[8%]">Cat</th>
              <th class="text-left font-normal px-3 py-2">Command</th>
              <th class="text-right font-normal px-3 py-2 w-[9%]">Calls</th>
              <th class="text-right font-normal px-3 py-2 w-[11%]">Tokens</th>
              <th class="text-left font-normal px-3 py-2 w-[20%]">Share</th>
              <th class="text-right font-normal px-3 py-2 w-[12%]">Cost</th>
              <th class="text-right font-normal pl-3 pr-5 py-2 w-[9%]">% block</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-border/40">
            {#each filteredCmds as c}
              {@const isOpen = expanded.has(c.group_key)}
              {@const hasMore = c.variants.length > 1}
              <tr
                class="hover:bg-panel2/60 {hasMore ? 'cursor-pointer' : ''}"
                onclick={() => hasMore && toggle(c.group_key)}
              >
                <td class="pl-5 pr-1 py-2.5 text-muted text-xs select-none">
                  {hasMore ? (isOpen ? "▾" : "▸") : ""}
                </td>
                <td class="pr-3 py-2.5">
                  <span class="text-xs uppercase tracking-wider {catColor(c.category)}">{c.category}</span>
                </td>
                <td class="px-3 py-2.5 truncate font-mono text-ink" title={redactText(c.cmd)}>{redactText(c.group_key)}</td>
                <td class="px-3 py-2.5 text-right num">{fmtInt(c.count)}</td>
                <td class="px-3 py-2.5 text-right num">{fmtTok(c.tokens)}</td>
                <td class="px-3 py-2.5">
                  <div class="flex items-center gap-2">
                    <div class="bar flex-1">
                      <div class="fill" style="width: {maxCmdTokens > 0 ? (c.tokens / maxCmdTokens) * 100 : 0}%"></div>
                    </div>
                    <div class="num text-xs text-muted w-9 text-right shrink-0">
                      {cmdTotals.tokens > 0 ? fmtPct((c.tokens / cmdTotals.tokens) * 100, 0) : "—"}
                    </div>
                  </div>
                </td>
                <td class="px-3 py-2.5 text-right num text-muted">{fmtUsd(c.cost_usd)}</td>
                <td class="pl-3 pr-5 py-2.5 text-right num text-muted">{blockPct(c.tokens)}</td>
              </tr>
              {#if isOpen && hasMore}
                <tr class="bg-panel2/30">
                  <td></td>
                  <td colspan="7" class="pl-3 pr-5 py-2">
                    <div class="text-[11px] uppercase tracking-wider text-muted mb-1.5">
                      Variants in <span class="font-mono normal-case tracking-normal">{redactText(c.group_key)}</span>
                      <span class="text-muted/70">· {c.variants.length} unique</span>
                    </div>
                    <div class="space-y-1">
                      {#each c.variants as v}
                        <div class="flex items-baseline gap-3">
                          <div class="num text-xs text-muted w-12 shrink-0 text-right">{fmtInt(v.count)}×</div>
                          <div class="font-mono text-xs text-ink min-w-0 break-all">{redactText(v.cmd)}</div>
                        </div>
                      {/each}
                    </div>
                  </td>
                </tr>
              {/if}
            {/each}
          </tbody>
        </table>
      {/if}
    </div>
  {/if}
</div>
