<script lang="ts">
  import { onMount } from 'svelte';
  import { cancelCapture } from '../lib/capture';

  let { seconds }: { seconds: number } = $props();
  // The badge window is created per countdown, so the initial length is all we need.
  // svelte-ignore state_referenced_locally
  let remaining = $state(seconds);

  onMount(() => {
    const id = setInterval(() => {
      remaining = Math.max(0, remaining - 1);
      if (remaining === 0) clearInterval(id);
    }, 1000);
    return () => clearInterval(id);
  });
  const cancel = () => void cancelCapture();
</script>

<svelte:window onkeydown={(e) => { if (e.key === 'Escape') cancel(); }} />

<button type="button" class="badge" aria-label="Capture in {remaining} seconds. Click or press Escape to cancel." onclick={cancel}>
  <svg viewBox="0 0 120 120" aria-hidden="true">
    <circle cx="60" cy="60" r="54" class="ring" />
    <circle cx="60" cy="60" r="54" class="progress" pathLength="1" style:stroke-dashoffset={1 - remaining / seconds} />
  </svg>
  <span class="count" data-testid="count">{remaining}</span>
  <span class="hint">Click to cancel</span>
</button>

<style>
  :global(html), :global(body) { background: #2a2a2d; margin: 0; overflow: hidden; } /* the badge window is opaque (no transparency flag in Rust) */
  .badge {
    position: fixed; inset: 0; border: 0; padding: 0; background: transparent; cursor: pointer;
    display: grid; place-items: center; color: #ececee;
    font-family: system-ui, -apple-system, 'Segoe UI Variable', Cantarell, Ubuntu, 'Noto Sans', sans-serif;
  }
  svg { position: absolute; inset: 0; width: 100%; height: 100%; }
  .ring { fill: none; stroke: rgb(255 255 255 / 0.18); stroke-width: 4; }
  .progress { fill: none; stroke: var(--accent, #0a64d8); stroke-width: 4; stroke-linecap: round; stroke-dasharray: 1; transform: rotate(-90deg); transform-origin: 60px 60px; }
  .count { position: relative; font-size: 44px; font-weight: 600; font-variant-numeric: tabular-nums; line-height: 1; margin-top: -8px; }
  .hint { position: absolute; bottom: 24px; font-size: 11px; color: #98989d; }
  .badge:focus-visible { outline: 2px solid var(--accent, #0a64d8); outline-offset: -4px; border-radius: 50%; }
</style>
