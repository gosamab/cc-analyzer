<script lang="ts">
  import { donutPalette } from "./palette";

  type Slice = { label: string; value: number; color?: string };
  let {
    slices,
    size = 140,
    thickness = 18,
    centerLabel = "",
    centerSub = "",
    onhover,
  }: {
    slices: Slice[];
    size?: number;
    thickness?: number;
    centerLabel?: string;
    centerSub?: string;
    onhover?: (i: number | null) => void;
  } = $props();

  const palette = donutPalette;

  const total = $derived(slices.reduce((a, s) => a + Math.max(s.value, 0), 0));
  const cx = $derived(size / 2);
  // Inset by 1px so the inter-slice stroke isn't clipped at the SVG edge.
  const r = $derived(size / 2 - 1);
  const inner = $derived(r - thickness);

  let hovered = $state<number | null>(null);
  function setHover(i: number | null) {
    hovered = i;
    onhover?.(i);
  }

  function arc(start: number, end: number, ri: number, ro: number) {
    if (end - start >= 1 - 1e-9) {
      const ax = cx + ro, ay = cx;
      const bx = cx - ro, by = cx;
      const ix = cx + ri, iy = cx;
      const jx = cx - ri, jy = cx;
      return (
        `M ${ax} ${ay} A ${ro} ${ro} 0 1 1 ${bx} ${by} ` +
        `A ${ro} ${ro} 0 1 1 ${ax} ${ay} Z ` +
        `M ${ix} ${iy} A ${ri} ${ri} 0 1 0 ${jx} ${jy} ` +
        `A ${ri} ${ri} 0 1 0 ${ix} ${iy} Z`
      );
    }
    const a0 = start * Math.PI * 2 - Math.PI / 2;
    const a1 = end * Math.PI * 2 - Math.PI / 2;
    const large = end - start > 0.5 ? 1 : 0;
    const x0 = cx + ro * Math.cos(a0);
    const y0 = cx + ro * Math.sin(a0);
    const x1 = cx + ro * Math.cos(a1);
    const y1 = cx + ro * Math.sin(a1);
    const x2 = cx + ri * Math.cos(a1);
    const y2 = cx + ri * Math.sin(a1);
    const x3 = cx + ri * Math.cos(a0);
    const y3 = cx + ri * Math.sin(a0);
    return `M ${x0} ${y0} A ${ro} ${ro} 0 ${large} 1 ${x1} ${y1} L ${x2} ${y2} A ${ri} ${ri} 0 ${large} 0 ${x3} ${y3} Z`;
  }

  const segments = $derived.by(() => {
    if (total <= 0) return [];
    let acc = 0;
    return slices.map((s, i) => {
      const start = acc / total;
      acc += Math.max(s.value, 0);
      const end = acc / total;
      return {
        i,
        start,
        end,
        color: s.color ?? palette[i % palette.length],
        d: arc(start, end, inner, r),
      };
    });
  });

</script>

<svg viewBox="0 0 {size} {size}" width={size} height={size} class="shrink-0">
  {#if segments.length === 0}
    <circle cx={cx} cy={cx} r={r - 1} fill="none" class="stroke-border" stroke-width="1" />
  {/if}
  {#each segments as seg}
    <path
      role="img"
      aria-label={slices[seg.i].label}
      d={seg.d}
      fill={seg.color}
      fill-opacity={hovered === null || hovered === seg.i ? 1 : 0.35}
      class="stroke-ink"
      stroke-width="1"
      onmouseenter={() => setHover(seg.i)}
      onmouseleave={() => setHover(null)}
      style="cursor: pointer; transition: fill-opacity 120ms ease"
    />
  {/each}
  {#if centerLabel}
    <text x={cx} y={cx - 2} text-anchor="middle" class="fill-ink num" style="font-size: 14px; font-weight: 600">
      {centerLabel}
    </text>
  {/if}
  {#if centerSub}
    <text x={cx} y={cx + 14} text-anchor="middle" class="fill-muted num" style="font-size: 10px">
      {centerSub}
    </text>
  {/if}
</svg>
