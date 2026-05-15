<script lang="ts">
  let { value, max = 100, color = "" }: { value: number; max?: number; color?: string } = $props();
  const pct = $derived(Math.max(0, Math.min(100, (value / max) * 100)));
</script>

<div class="bar">
  <div class="fill {color}" style="width: {pct}%"></div>
</div>
