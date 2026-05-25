<script lang="ts">
  import {
    ipc,
    type CommandRow,
    type ToolUsageRow,
    type BlockUsage,
    type CommandCategory,
    type SkillUsageRow,
    type McpUsageRow,
    type SlashCommandRow,
  } from "../ipc";
  import { fmtUsd, fmtTok, fmtInt, fmtPct, isoRange, impactOfLimit, redactText } from "../format";
  import { ui } from "../store.svelte";
  import RangePicker from "../components/RangePicker.svelte";

  let tools = $state<ToolUsageRow[]>([]);
  let commands = $state<CommandRow[]>([]);
  let skills = $state<SkillUsageRow[]>([]);
  let mcps = $state<McpUsageRow[]>([]);
  let slashes = $state<SlashCommandRow[]>([]);
  let block = $state<BlockUsage | null>(null);
  let loadingTools = $state(true);
  let loadingCmds = $state(true);
  let loadingSkills = $state(true);
  let loadingMcps = $state(true);
  let loadingSlash = $state(true);
  let tab = $state<"tools" | "bash" | "skills" | "mcps" | "slash">("tools");
  let mcpGroup = $state<"tool" | "server">("tool");
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
    loadingTools = loadingCmds = loadingSkills = loadingMcps = loadingSlash = true;

    ipc.toolUsage(since, until)
      .then((x) => { if (mine === reqId) tools = x; })
      .catch(console.error)
      .finally(() => { if (mine === reqId) loadingTools = false; });

    ipc.topCommands(since, until, 50)
      .then((x) => { if (mine === reqId) commands = x; })
      .catch(console.error)
      .finally(() => { if (mine === reqId) loadingCmds = false; });

    ipc.skillUsage(since, until)
      .then((x) => { if (mine === reqId) skills = x; })
      .catch(console.error)
      .finally(() => { if (mine === reqId) loadingSkills = false; });

    ipc.mcpUsage(since, until)
      .then((x) => { if (mine === reqId) mcps = x; })
      .catch(console.error)
      .finally(() => { if (mine === reqId) loadingMcps = false; });

    ipc.slashCommandUsage(since, until)
      .then((x) => { if (mine === reqId) slashes = x; })
      .catch(console.error)
      .finally(() => { if (mine === reqId) loadingSlash = false; });

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

  let skillTotals = $derived({
    count: skills.reduce((s, x) => s + x.count, 0),
    tokens: skills.reduce((s, x) => s + x.tokens, 0),
    cost: skills.reduce((s, x) => s + x.cost_usd, 0),
  });
  let maxSkillTokens = $derived(skills.reduce((m, x) => Math.max(m, x.tokens), 0));

  type McpDisplayRow = {
    label: string;
    sub: string;
    count: number;
    tokens: number;
    cost_usd: number;
    turns: number;
    sessions: number;
  };
  let mcpRows = $derived<McpDisplayRow[]>(
    mcpGroup === "tool"
      ? mcps.map((m) => ({
          label: m.short || m.tool,
          sub: m.server,
          count: m.count,
          tokens: m.tokens,
          cost_usd: m.cost_usd,
          turns: m.turns,
          sessions: m.sessions,
        }))
      : Object.values(
          mcps.reduce<Record<string, McpDisplayRow>>((acc, m) => {
            const k = m.server;
            const e = acc[k] ?? {
              label: k,
              sub: "",
              count: 0,
              tokens: 0,
              cost_usd: 0,
              turns: 0,
              sessions: 0,
            };
            e.count += m.count;
            e.tokens += m.tokens;
            e.cost_usd += m.cost_usd;
            e.turns += m.turns;
            e.sessions = Math.max(e.sessions, m.sessions);
            acc[k] = e;
            return acc;
          }, {}),
        ).sort((a, b) => b.tokens - a.tokens),
  );
  let mcpTotals = $derived({
    count: mcpRows.reduce((s, x) => s + x.count, 0),
    tokens: mcpRows.reduce((s, x) => s + x.tokens, 0),
    cost: mcpRows.reduce((s, x) => s + x.cost_usd, 0),
  });
  let maxMcpTokens = $derived(mcpRows.reduce((m, x) => Math.max(m, x.tokens), 0));

  let slashTotals = $derived({
    count: slashes.reduce((s, x) => s + x.count, 0),
    tokens: 0,
    cost: 0,
  });
  let maxSlashCount = $derived(slashes.reduce((m, x) => Math.max(m, x.count), 0));
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
        >Bash</button>
        <button
          class="px-3 py-1 rounded {tab === 'skills' ? 'bg-panel2 text-ink' : 'text-muted hover:text-ink'}"
          onclick={() => (tab = "skills")}
        >Skills</button>
        <button
          class="px-3 py-1 rounded {tab === 'mcps' ? 'bg-panel2 text-ink' : 'text-muted hover:text-ink'}"
          onclick={() => (tab = "mcps")}
        >MCPs</button>
        <button
          class="px-3 py-1 rounded {tab === 'slash' ? 'bg-panel2 text-ink' : 'text-muted hover:text-ink'}"
          onclick={() => (tab = "slash")}
        >Slash</button>
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
  {:else if tab === "bash"}
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
  {:else if tab === "skills"}
    <div class="card !p-0 overflow-hidden">
      <div class="px-5 py-4 border-b border-border">
        {@render statStrip(skillTotals, loadingSkills)}
      </div>
      {#if !loadingSkills && !skills.length}
        <div class="p-6 text-muted text-sm">No skill invocations in this range.</div>
      {:else if !loadingSkills}
        <table class="w-full text-sm table-fixed">
          <thead>
            <tr class="text-xs uppercase tracking-wider text-muted">
              <th class="text-left font-normal px-5 py-2 w-[30%]">Skill</th>
              <th class="text-right font-normal px-3 py-2 w-[9%]">Calls</th>
              <th class="text-right font-normal px-3 py-2 w-[9%]">Turns</th>
              <th class="text-right font-normal px-3 py-2 w-[9%]">Sessions</th>
              <th class="text-right font-normal px-3 py-2 w-[11%]">Tokens</th>
              <th class="text-left font-normal px-3 py-2">Share</th>
              <th class="text-right font-normal px-3 py-2 w-[12%]">Cost</th>
              <th class="text-right font-normal pl-3 pr-5 py-2 w-[9%]">% block</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-border/40">
            {#each skills as s}
              <tr class="hover:bg-panel2/60">
                <td class="px-5 py-2.5 truncate text-ink font-mono" title={s.skill}>{s.skill}</td>
                <td class="px-3 py-2.5 text-right num">{fmtInt(s.count)}</td>
                <td class="px-3 py-2.5 text-right num text-muted">{fmtInt(s.turns)}</td>
                <td class="px-3 py-2.5 text-right num text-muted">{fmtInt(s.sessions)}</td>
                <td class="px-3 py-2.5 text-right num">{fmtTok(s.tokens)}</td>
                <td class="px-3 py-2.5">
                  <div class="flex items-center gap-2">
                    <div class="bar flex-1">
                      <div class="fill" style="width: {maxSkillTokens > 0 ? (s.tokens / maxSkillTokens) * 100 : 0}%"></div>
                    </div>
                    <div class="num text-xs text-muted w-9 text-right shrink-0">
                      {skillTotals.tokens > 0 ? fmtPct((s.tokens / skillTotals.tokens) * 100, 0) : "—"}
                    </div>
                  </div>
                </td>
                <td class="px-3 py-2.5 text-right num text-muted">{fmtUsd(s.cost_usd)}</td>
                <td class="pl-3 pr-5 py-2.5 text-right num text-muted">{blockPct(s.tokens)}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    </div>
  {:else if tab === "mcps"}
    <div class="card !p-0 overflow-hidden">
      <div class="px-5 py-4 border-b border-border flex items-center justify-between gap-4">
        <div class="min-w-0">{@render statStrip(mcpTotals, loadingMcps)}</div>
        <div class="flex gap-1 text-xs">
          <button
            class="px-2.5 py-1 rounded border {mcpGroup === 'tool' ? 'bg-panel2 text-ink border-border' : 'text-muted border-transparent hover:text-ink hover:border-border'}"
            onclick={() => (mcpGroup = "tool")}
          >by tool</button>
          <button
            class="px-2.5 py-1 rounded border {mcpGroup === 'server' ? 'bg-panel2 text-ink border-border' : 'text-muted border-transparent hover:text-ink hover:border-border'}"
            onclick={() => (mcpGroup = "server")}
          >by server</button>
        </div>
      </div>
      {#if !loadingMcps && !mcpRows.length}
        <div class="p-6 text-muted text-sm">No MCP tool calls in this range.</div>
      {:else if !loadingMcps}
        <table class="w-full text-sm table-fixed">
          <thead>
            <tr class="text-xs uppercase tracking-wider text-muted">
              <th class="text-left font-normal px-5 py-2 w-[18%]">{mcpGroup === "tool" ? "Tool" : "Server"}</th>
              <th class="text-left font-normal pr-3 py-2 w-[18%]">{mcpGroup === "tool" ? "Server" : ""}</th>
              <th class="text-right font-normal px-3 py-2 w-[9%]">Calls</th>
              <th class="text-right font-normal px-3 py-2 w-[9%]">Turns</th>
              <th class="text-right font-normal px-3 py-2 w-[11%]">Tokens</th>
              <th class="text-left font-normal px-3 py-2">Share</th>
              <th class="text-right font-normal px-3 py-2 w-[12%]">Cost</th>
              <th class="text-right font-normal pl-3 pr-5 py-2 w-[9%]">% block</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-border/40">
            {#each mcpRows as m}
              <tr class="hover:bg-panel2/60">
                <td class="px-5 py-2.5 truncate text-ink font-mono" title={m.label}>{m.label}</td>
                <td class="pr-3 py-2.5 truncate text-muted text-xs" title={m.sub}>{m.sub}</td>
                <td class="px-3 py-2.5 text-right num">{fmtInt(m.count)}</td>
                <td class="px-3 py-2.5 text-right num text-muted">{fmtInt(m.turns)}</td>
                <td class="px-3 py-2.5 text-right num">{fmtTok(m.tokens)}</td>
                <td class="px-3 py-2.5">
                  <div class="flex items-center gap-2">
                    <div class="bar flex-1">
                      <div class="fill" style="width: {maxMcpTokens > 0 ? (m.tokens / maxMcpTokens) * 100 : 0}%"></div>
                    </div>
                    <div class="num text-xs text-muted w-9 text-right shrink-0">
                      {mcpTotals.tokens > 0 ? fmtPct((m.tokens / mcpTotals.tokens) * 100, 0) : "—"}
                    </div>
                  </div>
                </td>
                <td class="px-3 py-2.5 text-right num text-muted">{fmtUsd(m.cost_usd)}</td>
                <td class="pl-3 pr-5 py-2.5 text-right num text-muted">{blockPct(m.tokens)}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    </div>
  {:else if tab === "slash"}
    <div class="card !p-0 overflow-hidden">
      <div class="px-5 py-4 border-b border-border">
        {@render statStrip(slashTotals, loadingSlash)}
      </div>
      {#if !loadingSlash && !slashes.length}
        <div class="p-6 text-muted text-sm">No slash commands in this range.</div>
      {:else if !loadingSlash}
        <table class="w-full text-sm table-fixed">
          <thead>
            <tr class="text-xs uppercase tracking-wider text-muted">
              <th class="text-left font-normal px-5 py-2 w-[30%]">Command</th>
              <th class="text-right font-normal px-3 py-2 w-[12%]">Uses</th>
              <th class="text-right font-normal px-3 py-2 w-[14%]">Sessions</th>
              <th class="text-left font-normal pl-3 pr-5 py-2">Share</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-border/40">
            {#each slashes as s}
              <tr class="hover:bg-panel2/60">
                <td class="px-5 py-2.5 truncate text-ink font-mono" title={s.cmd}>{s.cmd}</td>
                <td class="px-3 py-2.5 text-right num">{fmtInt(s.count)}</td>
                <td class="px-3 py-2.5 text-right num text-muted">{fmtInt(s.sessions)}</td>
                <td class="pl-3 pr-5 py-2.5">
                  <div class="flex items-center gap-2">
                    <div class="bar flex-1">
                      <div class="fill" style="width: {maxSlashCount > 0 ? (s.count / maxSlashCount) * 100 : 0}%"></div>
                    </div>
                    <div class="num text-xs text-muted w-9 text-right shrink-0">
                      {slashTotals.count > 0 ? fmtPct((s.count / slashTotals.count) * 100, 0) : "—"}
                    </div>
                  </div>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    </div>
  {/if}
</div>
