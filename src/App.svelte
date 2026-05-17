<script lang="ts">
  import { ui, currency, theme, privacy } from "./lib/store.svelte";

  currency.load();
  theme.load();
  privacy.load();
  import { ipc } from "./lib/ipc";
  import Dashboard from "./lib/views/Dashboard.svelte";
  import Explorer from "./lib/views/Explorer.svelte";
  import Insights from "./lib/views/Insights.svelte";
  import Commands from "./lib/views/Commands.svelte";
  import Settings from "./lib/views/Settings.svelte";
  import { onMount } from "svelte";

  let refreshing = $state(false);
  let lastRefresh = $state<string | null>(null);
  let newRows = $state(0);

  async function refresh() {
    refreshing = true;
    try {
      newRows = await ipc.refreshLogs();
      lastRefresh = new Date().toLocaleTimeString();
    } catch (e) {
      console.error(e);
    } finally {
      refreshing = false;
    }
  }

  onMount(refresh);

  const tabs: { id: typeof ui.view; label: string }[] = [
    { id: "dashboard", label: "Dashboard" },
    { id: "explorer", label: "Explorer" },
    { id: "insights", label: "Recommendations" },
    { id: "commands", label: "Commands" },
    { id: "settings", label: "Settings" },
  ];

  // Lazy-mount each view on first visit, then keep alive so tab switches are instant.
  let mounted = $state<Record<string, boolean>>({ dashboard: true });
  $effect(() => {
    mounted[ui.view] = true;
  });
</script>

<div class="h-full flex flex-col">
  <header class="flex items-center justify-between px-4 h-12 border-b border-border bg-panel">
    <div class="flex items-center gap-6">
      <button
        class="font-semibold tracking-tight hover:text-accent transition-colors"
        onclick={() => ui.open("dashboard")}
        title="Go to Dashboard"
      >
        Claude Code Analyzer
      </button>
      <nav class="flex gap-1">
        {#each tabs as t}
          <button
            class="px-3 py-1 text-sm rounded {ui.view === t.id ? 'bg-panel2 text-ink' : 'text-muted hover:text-ink'}"
            onclick={() => ui.open(t.id)}
          >
            {t.label}
          </button>
        {/each}
      </nav>
    </div>
    <div class="flex items-center gap-3 text-xs text-muted">
      {#if lastRefresh}
        <span>refreshed {lastRefresh} · +{newRows} rows</span>
      {/if}
      <button
        class="btn !px-2 {privacy.enabled ? '!text-accent !border-accent/40' : ''}"
        onclick={() => privacy.toggle()}
        title={privacy.enabled ? "Privacy mode on — paths & project names redacted" : "Turn on privacy mode (redact paths & projects)"}
        aria-label="Toggle privacy mode"
      >
        {privacy.enabled ? "⊘" : "⊙"}
      </button>
      <button
        class="btn !px-2"
        onclick={() => theme.toggle()}
        title={theme.mode === "dark" ? "Switch to light mode" : "Switch to dark mode"}
        aria-label="Toggle theme"
      >
        {theme.mode === "dark" ? "☀" : "☾"}
      </button>
      <button class="btn" onclick={refresh} disabled={refreshing}>
        {refreshing ? "Refreshing…" : "Refresh"}
      </button>
    </div>
  </header>

  <main class="flex-1 overflow-hidden relative">
    <div class="absolute inset-0 overflow-auto" class:hidden={ui.view !== "dashboard"}>{#if mounted.dashboard}<Dashboard />{/if}</div>
    <div class="absolute inset-0 overflow-hidden" class:hidden={ui.view !== "explorer"}>{#if mounted.explorer}<Explorer />{/if}</div>
    <div class="absolute inset-0 overflow-auto" class:hidden={ui.view !== "insights"}>{#if mounted.insights}<Insights />{/if}</div>
    <div class="absolute inset-0 overflow-auto" class:hidden={ui.view !== "commands"}>{#if mounted.commands}<Commands />{/if}</div>
    <div class="absolute inset-0 overflow-auto" class:hidden={ui.view !== "settings"}>{#if mounted.settings}<Settings />{/if}</div>
  </main>
</div>
