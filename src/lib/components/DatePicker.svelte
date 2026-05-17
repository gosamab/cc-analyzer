<script lang="ts">
  let {
    value = $bindable<string>(""),
    max,
    min,
    align = "right",
    children,
  }: {
    value?: string;
    max?: string;
    min?: string;
    align?: "left" | "right";
    children?: import("svelte").Snippet;
  } = $props();

  let open = $state(false);
  let popoverEl: HTMLDivElement | undefined = $state();
  let triggerEl: HTMLButtonElement | undefined = $state();
  let popX = $state(0);
  let popY = $state(0);

  // Recompute popover position relative to viewport. position:fixed lets us escape
  // any `overflow:hidden` ancestor (like the segmented control that wraps the trigger).
  function placePopover() {
    if (!triggerEl) return;
    const r = triggerEl.getBoundingClientRect();
    const popWidth = 256; // matches w-64
    if (align === "right") {
      popX = r.right - popWidth;
    } else {
      popX = r.left;
    }
    // Clamp horizontally so it stays on-screen.
    popX = Math.max(8, Math.min(popX, window.innerWidth - popWidth - 8));
    popY = r.bottom + 4;
  }

  // Calendar nav state — initialise to the selected month on each open.
  let viewYear = $state<number>(new Date().getFullYear());
  let viewMonth = $state<number>(new Date().getMonth());

  function parseISO(s?: string): Date | null {
    if (!s) return null;
    const [y, m, d] = s.slice(0, 10).split("-").map((x) => parseInt(x, 10));
    if (!y || !m || !d) return null;
    return new Date(y, m - 1, d);
  }
  function toISO(d: Date) {
    const yyyy = d.getFullYear();
    const mm = String(d.getMonth() + 1).padStart(2, "0");
    const dd = String(d.getDate()).padStart(2, "0");
    return `${yyyy}-${mm}-${dd}`;
  }

  function openPopover() {
    const d = parseISO(value) ?? new Date();
    viewYear = d.getFullYear();
    viewMonth = d.getMonth();
    placePopover();
    open = true;
  }

  function close() {
    open = false;
  }

  function pick(d: Date) {
    value = toISO(d);
    close();
  }

  function prevMonth() {
    if (viewMonth === 0) {
      viewMonth = 11;
      viewYear -= 1;
    } else {
      viewMonth -= 1;
    }
  }
  function nextMonth() {
    if (viewMonth === 11) {
      viewMonth = 0;
      viewYear += 1;
    } else {
      viewMonth += 1;
    }
  }

  const monthName = $derived(
    new Date(viewYear, viewMonth, 1).toLocaleString(undefined, { month: "long", year: "numeric" })
  );

  // 6 weeks × 7 days, padded with prev/next month days so the grid is always full.
  const grid = $derived.by(() => {
    const first = new Date(viewYear, viewMonth, 1);
    const startWeekday = first.getDay(); // 0=Sun
    const start = new Date(viewYear, viewMonth, 1 - startWeekday);
    const days: { date: Date; inMonth: boolean; disabled: boolean }[] = [];
    const maxD = parseISO(max);
    const minD = parseISO(min);
    for (let i = 0; i < 42; i++) {
      const d = new Date(start.getFullYear(), start.getMonth(), start.getDate() + i);
      const disabled =
        (maxD ? d > maxD : false) || (minD ? d < minD : false);
      days.push({
        date: d,
        inMonth: d.getMonth() === viewMonth,
        disabled,
      });
    }
    return days;
  });

  const selectedISO = $derived(value?.slice(0, 10) ?? "");
  const todayISO = toISO(new Date());

  function onWindowDown(e: MouseEvent) {
    if (!open) return;
    const t = e.target as Node;
    if (popoverEl?.contains(t) || triggerEl?.contains(t)) return;
    close();
  }
  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape" && open) close();
  }
</script>

<svelte:window
  onmousedown={onWindowDown}
  onkeydown={onKey}
  onresize={() => open && placePopover()}
  onscroll={() => open && placePopover()}
/>

<div class="relative inline-block">
  <button
    bind:this={triggerEl}
    type="button"
    onclick={(e) => { e.stopPropagation(); open ? close() : openPopover(); }}
    class="inline-flex items-center cursor-pointer bg-transparent border-0 p-0 m-0 text-inherit"
  >
    {@render children?.()}
  </button>

  {#if open}
    <div
      bind:this={popoverEl}
      class="fixed z-50 bg-panel border border-border rounded-md shadow-xl p-2 w-64 select-none"
      style="left: {popX}px; top: {popY}px;"
    >
      <!-- Header -->
      <div class="flex items-center justify-between mb-2">
        <button
          type="button"
          class="px-2 py-1 text-xs text-muted hover:text-ink rounded hover:bg-panel2"
          onclick={prevMonth}
          aria-label="Previous month"
        >‹</button>
        <div class="text-sm font-medium">{monthName}</div>
        <button
          type="button"
          class="px-2 py-1 text-xs text-muted hover:text-ink rounded hover:bg-panel2"
          onclick={nextMonth}
          aria-label="Next month"
        >›</button>
      </div>

      <!-- Day-of-week row -->
      <div class="grid grid-cols-7 gap-px mb-1 text-[10px] text-muted text-center">
        {#each ["S","M","T","W","T","F","S"] as d}
          <div>{d}</div>
        {/each}
      </div>

      <!-- Days -->
      <div class="grid grid-cols-7 gap-px text-xs">
        {#each grid as cell}
          {@const iso = toISO(cell.date)}
          {@const isSelected = iso === selectedISO}
          {@const isToday = iso === todayISO}
          <button
            type="button"
            disabled={cell.disabled}
            onclick={() => pick(cell.date)}
            class="num h-7 rounded flex items-center justify-center
              {isSelected ? 'bg-accent/90 text-bg' :
               cell.disabled ? 'text-muted/30 cursor-not-allowed' :
               cell.inMonth ? 'text-ink hover:bg-panel2' : 'text-muted/50 hover:bg-panel2'}
              {isToday && !isSelected ? 'ring-1 ring-accent/60' : ''}
            "
          >
            {cell.date.getDate()}
          </button>
        {/each}
      </div>

      <!-- Footer shortcuts -->
      <div class="mt-2 pt-2 border-t border-border flex items-center justify-between text-xs">
        <button
          type="button"
          class="text-muted hover:text-ink"
          onclick={() => { const d = new Date(); d.setDate(d.getDate() - 1); pick(d); }}
        >
          Yesterday
        </button>
        <button
          type="button"
          class="text-muted hover:text-ink"
          onclick={() => pick(new Date())}
        >
          Today
        </button>
      </div>
    </div>
  {/if}
</div>
