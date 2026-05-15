<script lang="ts">
  import { ipc, type Recommendation, type HealthSignal } from "../ipc";
  import { ui } from "../store.svelte";
  import { fmtUsd, fmtTok, isoRange, shortProject } from "../format";
  import RangePicker from "../components/RangePicker.svelte";

  let recs = $state<Recommendation[]>([]);
  let healthy = $state<HealthSignal[]>([]);
  let loadingRecs = $state(true);
  let loadingHealth = $state(true);

  let reqId = 0;
  $effect(() => {
    const r = ui.range;
    const { since, until } = isoRange(r);
    const mine = ++reqId;
    loadingRecs = loadingHealth = true;

    ipc.recommendations(since, until)
      .then((x) => { if (mine === reqId) recs = x; })
      .catch(console.error)
      .finally(() => { if (mine === reqId) loadingRecs = false; });

    ipc.healthSignals(since, until)
      .then((x) => { if (mine === reqId) healthy = x; })
      .catch(console.error)
      .finally(() => { if (mine === reqId) loadingHealth = false; });
  });

  const sevPill = (s: string) =>
    s === "HIGH" ? "pill-err" : s === "MED" ? "pill-warn" : "pill-ok";

  function openSession(id: string) {
    ui.drillSession(id);
  }
  function openProject(p: string) {
    ui.drillProject(p);
  }
</script>

<div class="p-6 space-y-6">
  <div class="flex items-center justify-between">
    <h1 class="text-lg font-semibold">Insights</h1>
    <RangePicker />
  </div>

  {#if loadingRecs}
    <div class="text-muted text-sm">Analyzing…</div>
  {:else}
    {#if !recs.length}
      <div class="card text-sm text-muted">
        No recommendations triggered for this range — looks healthy.
      </div>
    {:else}
      <div class="space-y-3">
        {#each recs as r}
          <div class="card">
            <div class="flex items-center gap-3">
              <span class="pill {sevPill(r.severity)}">{r.severity}</span>
              <div class="font-medium">{r.title}</div>
              <div class="ml-auto text-sm num text-right">
                {#if r.estimated_savings_tokens > 0}
                  <div>~{fmtTok(r.estimated_savings_tokens)} tokens recoverable</div>
                {/if}
                {#if r.estimated_savings_usd > 0}
                  <div class="text-muted text-xs">~{fmtUsd(r.estimated_savings_usd)} cost impact</div>
                {/if}
              </div>
            </div>
            <p class="mt-2 text-sm text-muted">{r.body}</p>

            <div class="mt-3 rounded border border-border bg-panel2 p-3">
              <div class="text-xs uppercase tracking-wide text-muted mb-1">Next action</div>
              <div class="text-sm text-ink">{r.action}</div>
              {#if r.action_session_id || r.action_project}
                <div class="mt-2 flex gap-2">
                  {#if r.action_session_id}
                    <button
                      class="btn text-xs px-2 py-1"
                      onclick={() => openSession(r.action_session_id!)}
                    >
                      Open session →
                    </button>
                  {/if}
                  {#if r.action_project}
                    <button
                      class="btn text-xs px-2 py-1"
                      onclick={() => openProject(r.action_project!)}
                    >
                      Filter Explorer to {shortProject(r.action_project)} →
                    </button>
                  {/if}
                </div>
              {/if}
            </div>

            <details class="mt-2">
              <summary class="text-xs text-muted cursor-pointer">Evidence</summary>
              <pre class="mt-1 text-xs text-muted overflow-x-auto">{JSON.stringify(r.evidence, null, 2)}</pre>
            </details>
          </div>
        {/each}
      </div>
    {/if}

    {#if healthy.length}
      <div>
        <div class="text-sm font-medium text-muted mb-2">What's healthy</div>
        <div class="grid grid-cols-1 md:grid-cols-2 gap-3">
          {#each healthy as h}
            <div class="card">
              <div class="flex items-center gap-2">
                <span class="pill pill-ok">OK</span>
                <div class="font-medium text-sm">{h.title}</div>
              </div>
              <p class="mt-1 text-sm text-muted">{h.detail}</p>
            </div>
          {/each}
        </div>
      </div>
    {/if}
  {/if}
</div>
