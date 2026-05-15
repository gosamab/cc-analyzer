<script lang="ts">
  import { ui } from "../store.svelte";
  const ranges = [
    { v: 1, label: "Today" },
    { v: 7, label: "7d" },
    { v: 30, label: "30d" },
    { v: 90, label: "90d" },
  ] as const;
</script>

<div class="inline-flex bg-panel2 border border-border rounded-md overflow-hidden">
  {#each ranges as r}
    <button
      class="px-3 py-1 text-xs {ui.range === r.v ? 'bg-accent/90 text-bg' : 'text-muted hover:text-ink'}"
      onclick={() => (ui.range = r.v)}
    >
      {r.label}
    </button>
  {/each}
</div>
