<script lang="ts">
  const MONTHS = ['Jan','Feb','Mar','Apr','May','Jun','Jul','Aug','Sep','Oct','Nov','Dec'];
  const DAYS = ['Mon','Tue','Wed','Thu','Fri','Sat','Sun'];
  const TOTAL_WEEKS = 52;
  const GAP = 3;

  type DayCell = { day: string; value: number; secondary?: string };
  type Cell = { day: string; value: number; secondary?: string; inRange: boolean };
  type Week = { cells: Cell[]; monthLabel: string | null };

  let {
    days,
    formatValue = (v: number) => v.toLocaleString(),
  }: {
    days: DayCell[];
    formatValue?: (v: number) => string;
  } = $props();

  // Mon=0 … Sun=6
  function isoWeekday(d: Date): number {
    return (d.getDay() + 6) % 7;
  }

  function toLocalIso(d: Date): string {
    const y = d.getFullYear();
    const m = String(d.getMonth() + 1).padStart(2, '0');
    const dd = String(d.getDate()).padStart(2, '0');
    return `${y}-${m}-${dd}`;
  }

  const grid = $derived.by((): Week[] => {
    const map = new Map(days.map(d => [d.day, d]));

    const now = new Date();
    now.setHours(0, 0, 0, 0);

    // Last week column starts on the Monday of the current week
    const lastMonday = new Date(now);
    lastMonday.setDate(now.getDate() - isoWeekday(now));

    // First week column starts (TOTAL_WEEKS - 1) weeks earlier
    const startMonday = new Date(lastMonday);
    startMonday.setDate(lastMonday.getDate() - (TOTAL_WEEKS - 1) * 7);

    // Determine actual data range so we can mark inRange
    const sorted = [...days].sort((a, b) => a.day.localeCompare(b.day));
    const firstData = sorted.length ? new Date(sorted[0].day + 'T00:00:00') : now;

    const weeks: Week[] = [];
    let lastMonth = -1;

    for (let w = 0; w < TOTAL_WEEKS; w++) {
      const weekMonday = new Date(startMonday);
      weekMonday.setDate(startMonday.getDate() + w * 7);

      const cells: Cell[] = [];
      for (let i = 0; i < 7; i++) {
        const d = new Date(weekMonday);
        d.setDate(weekMonday.getDate() + i);
        const iso = toLocalIso(d);
        const data = map.get(iso);
        cells.push({
          day: iso,
          value: data?.value ?? 0,
          secondary: data?.secondary,
          inRange: d >= firstData && d <= now,
        });
      }

      const mon = weekMonday.getMonth();
      const monthLabel = mon !== lastMonth ? MONTHS[mon] : null;
      if (monthLabel) lastMonth = mon;

      weeks.push({ cells, monthLabel });
    }

    return weeks;
  });

  const maxVal = $derived(Math.max(...days.map(d => d.value), 1));

  let containerWidth = $state(0);

  // Cells fill the full container width at 52 fixed columns
  const cellSize = $derived.by(() => {
    if (!containerWidth) return 14;
    const DAY_LABELS_W = 40; // w-10
    const GAP = 3; // gap between cells/columns
    const available = containerWidth - DAY_LABELS_W - GAP * (TOTAL_WEEKS - 1);
    return Math.max(8, Math.floor(available / TOTAL_WEEKS));
  });

  function opacity(v: number): number {
    if (v === 0) return 0;
    return 0.12 + (v / maxVal) * 0.88;
  }

  function cellTitle(cell: Cell): string {
    if (!cell.inRange || cell.value === 0) return cell.day;
    const tok = formatValue(cell.value);
    return cell.secondary ? `${cell.day}: ${tok} · ${cell.secondary}` : `${cell.day}: ${tok}`;
  }
</script>

<div class="w-full" bind:clientWidth={containerWidth}>
  <div class="flex flex-col">

    <!-- Month labels -->
    <div class="flex pl-10 mb-1" style="gap: {GAP}px" >
      {#each grid as week}
        <div
          class="shrink-0 text-[10px] text-muted font-medium leading-none truncate"
          style="width: {cellSize}px"
        >
          {week.monthLabel ?? ''}
        </div>
      {/each}
    </div>

    <!-- Grid: day-of-week labels + week columns -->
    <div class="flex" style="gap: {GAP}px">
      <!-- Day labels -->
      <div class="flex flex-col w-10 shrink-0" style="gap: {GAP}px">
        {#each DAYS as d}
          <div
            class="text-[10px] text-muted text-right pr-1 flex items-center justify-end"
            style="height: {cellSize}px"
          >{d}</div>
        {/each}
      </div>

      <!-- Weeks -->
      {#each grid as week}
        <div class="flex flex-col shrink-0" style="gap: {GAP}px">
          {#each week.cells as cell}
            <div
              class="rounded-[2px] cursor-default"
              style="width: {cellSize}px; height: {cellSize}px; {
                cell.inRange
                  ? cell.value > 0
                    ? `background: rgb(var(--color-accent) / ${opacity(cell.value)})`
                    : 'background: rgb(var(--color-border) / 0.4)'
                  : 'background: transparent'
              }"
              title={cellTitle(cell)}
            ></div>
          {/each}
        </div>
      {/each}
    </div>

    <!-- Legend -->
    <div class="flex items-center mt-2 pl-10" style="gap: {GAP}px">
      <span class="text-[9px] text-muted mr-1">less</span>
      {#each [0.12, 0.35, 0.58, 0.80, 1.0] as op}
        <div
          class="rounded-[2px]"
          style="width: {cellSize}px; height: {cellSize}px; background: rgb(var(--color-accent) / {op})"
        ></div>
      {/each}
      <span class="text-[9px] text-muted ml-1">more</span>
    </div>

  </div>
</div>
