<script lang="ts">
  let {
    values,
    height = 28,
    color = "rgb(224 122 95)",
  }: { values: number[]; height?: number; color?: string } = $props();

  const max = $derived(Math.max(...values, 1));
  const w = 100;
  const h = $derived(height);
  const points = $derived(
    values.length === 0
      ? ""
      : values
          .map((v, i) => {
            const x = (i / Math.max(values.length - 1, 1)) * w;
            const y = h - (v / max) * h;
            return `${x.toFixed(1)},${y.toFixed(1)}`;
          })
          .join(" ")
  );
  const area = $derived(
    points ? `0,${h} ${points} ${w},${h}` : ""
  );
</script>

<svg viewBox="0 0 {w} {h}" preserveAspectRatio="none" class="w-full" style="height: {h}px">
  {#if values.length > 1}
    <polygon points={area} fill={color} fill-opacity="0.18" />
    <polyline points={points} fill="none" stroke={color} stroke-width="1.5" />
  {/if}
</svg>
