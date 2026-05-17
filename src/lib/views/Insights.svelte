<script lang="ts">
  import { ipc, type Recommendation, type HealthSignal, type BlockUsage } from "../ipc";
  import { ui } from "../store.svelte";
  import { fmtUsd, fmtTok, isoRange, shortProject, impactOfLimit, redactText } from "../format";
  import RangePicker from "../components/RangePicker.svelte";

  let recs = $state<Recommendation[]>([]);
  let healthy = $state<HealthSignal[]>([]);
  let block = $state<BlockUsage | null>(null);
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

    // Block limit isn't range-scoped, but tying its fetch to the same effect keeps it
    // fresh on view re-mount without an extra effect.
    ipc.blockUsage()
      .then((b) => { if (mine === reqId) block = b; })
      .catch(console.error);
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
    <h1 class="text-lg font-semibold">Recommendations</h1>
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
      <div class="space-y-2">
        {#each recs as r}
          <div class="card !py-3 !px-4">
            <div class="flex items-start gap-3">
              <span class="pill {sevPill(r.severity)} shrink-0 mt-0.5">{r.severity}</span>
              <div class="flex-1 min-w-0">
                <div class="font-medium text-sm">{r.title}</div>
                <div class="text-xs text-muted mt-1">{redactText(r.body)}</div>
                <div class="text-xs text-ink mt-2 flex items-start gap-1.5">
                  <span class="text-muted shrink-0">→</span>
                  <span class="min-w-0">{redactText(r.action)}</span>
                </div>
                {#if r.action_session_id || r.action_project}
                  <div class="mt-2 flex gap-1.5">
                    {#if r.action_session_id}
                      <button class="btn !py-0.5 !px-2 text-xs" onclick={() => openSession(r.action_session_id!)}>
                        Open session
                      </button>
                    {/if}
                    {#if r.action_project}
                      <button class="btn !py-0.5 !px-2 text-xs" onclick={() => openProject(r.action_project!)}>
                        Filter to {shortProject(r.action_project)}
                      </button>
                    {/if}
                  </div>
                {/if}
              </div>
              {#if r.estimated_savings_tokens > 0 || r.estimated_savings_usd > 0}
                <div class="text-right text-xs num shrink-0 leading-relaxed">
                  {#if r.estimated_savings_tokens > 0}
                    <div class="text-ink">~{fmtTok(r.estimated_savings_tokens)} tokens</div>
                    {#if block && impactOfLimit(r.estimated_savings_tokens, block.limit_tokens, true)}
                      <div class="text-muted">{impactOfLimit(r.estimated_savings_tokens, block.limit_tokens, true)} of block</div>
                    {/if}
                  {/if}
                  {#if r.estimated_savings_usd > 0}
                    <div class="text-muted">~{fmtUsd(r.estimated_savings_usd)}</div>
                  {/if}
                </div>
              {/if}
            </div>
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
