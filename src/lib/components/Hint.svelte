<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    text,
    side = "auto",
    children,
  }: {
    text: string;
    side?: "left" | "right" | "auto";
    children: Snippet;
  } = $props();

  let open = $state(false);
  let triggerEl: HTMLSpanElement | undefined = $state();
  let align = $state<"left" | "right">("right");

  $effect(() => {
    if (!open || !triggerEl) return;
    if (side !== "auto") {
      align = side;
      return;
    }
    const rect = triggerEl.getBoundingClientRect();
    // If trigger is in the right third of the viewport, anchor tooltip to the right
    // so it grows leftward and stays on-screen.
    align = rect.left > window.innerWidth * 0.6 ? "right" : "left";
  });
</script>

<span
  bind:this={triggerEl}
  role="button"
  tabindex="0"
  aria-describedby="hint-tooltip"
  class="relative inline-block"
  onmouseenter={() => (open = true)}
  onmouseleave={() => (open = false)}
  onfocusin={() => (open = true)}
  onfocusout={() => (open = false)}
>
  {@render children()}
  {#if open}
    <span
      role="tooltip"
      class="absolute z-50 bottom-full mb-1 w-64 whitespace-normal bg-panel border border-border rounded-md shadow-lg px-2 py-1 text-xs text-ink leading-relaxed"
      style={align === "right" ? "right: 0;" : "left: 0;"}
    >
      {text}
    </span>
  {/if}
</span>
