<script lang="ts">
  import { onMount } from 'svelte';
  import { captureState, type CaptureAvailability, type CaptureMode } from './capture';
  let { busy, capture, timer, settings }: {
    busy: boolean;
    capture: (mode: CaptureMode) => void;
    timer: (seconds: number) => void;
    settings: () => void;
  } = $props();
  const WAYLAND = 'Not available on Wayland: the system dialog chooses what to capture.';
  const TIMERS = [3, 5, 10];
  let open = $state(false);
  let timers = $state(false);
  let availability = $state<CaptureAvailability>({ wayland: false, previous: false });
  let root: HTMLDivElement;
  let toggle: HTMLButtonElement;

  async function refresh() {
    try { availability = await captureState(); } catch { /* keep the last known state */ }
  }
  function toggleMenu() {
    open = !open; timers = false;
    if (open) void refresh();
  }
  function close(refocus = false) {
    open = false; timers = false;
    if (refocus) toggle.focus();
  }
  function pick(action: () => void) { close(); action(); }
  function onKey(event: KeyboardEvent) {
    if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); close(true); return; }
    if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return;
    event.preventDefault();
    const items = Array.from(root.querySelectorAll<HTMLButtonElement>('[role="menuitem"]:not(:disabled)'));
    if (!items.length) return;
    const at = items.indexOf(document.activeElement as HTMLButtonElement);
    const next = event.key === 'ArrowDown' ? (at + 1) % items.length : (at <= 0 ? items.length - 1 : at - 1);
    items[next].focus();
  }
  onMount(() => { void refresh(); });
</script>
<svelte:window onpointerdown={(event) => { if (open && !root.contains(event.target as Node)) close(); }} />
<div class="capture" bind:this={root}>
  <button class="primary main" disabled={busy} title="Capture a region" onclick={() => capture('region')}>
    <svg width="16" height="16" viewBox="0 0 24 24" aria-hidden="true"><path d="M4 9V4h5M15 4h5v5M20 15v5h-5M9 20H4v-5" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" /><circle cx="12" cy="12" r="2.5" fill="currentColor" /></svg>
    Capture
  </button>
  <button class="primary more" aria-label="Capture options" aria-haspopup="menu" aria-expanded={open} disabled={busy} bind:this={toggle} onclick={toggleMenu}>
    <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><path d="M2 3.5l3 3 3-3" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" /></svg>
  </button>
  {#if open}
    <div class="menu" role="menu" aria-label="Capture options" tabindex="-1" onkeydown={onKey}>
      <button role="menuitem" onclick={() => pick(() => capture('window'))} disabled={availability.wayland} title={availability.wayland ? WAYLAND : undefined}>Window</button>
      <button role="menuitem" onclick={() => pick(() => capture('screen'))}>Full screen</button>
      <button role="menuitem" onclick={() => pick(() => capture('previous'))} disabled={availability.wayland || !availability.previous} title={availability.wayland ? WAYLAND : undefined}>Previous region</button>
      <button role="menuitem" aria-haspopup="menu" aria-expanded={timers} onclick={() => timers = !timers}>Timer<span aria-hidden="true"> ▸</span></button>
      {#if timers}
        {#each TIMERS as seconds (seconds)}
          <button role="menuitem" class="sub" onclick={() => pick(() => timer(seconds))}>{seconds} s</button>
        {/each}
      {/if}
      <hr />
      <button role="menuitem" onclick={() => pick(settings)}>Settings…</button>
    </div>
  {/if}
</div>
<style>
  .capture { position: relative; display: flex; }
  .main { display: inline-flex; align-items: center; gap: .35rem; border-top-right-radius: 0; border-bottom-right-radius: 0; }
  .more { min-width: 0; padding: 0 .5rem; border-top-left-radius: 0; border-bottom-left-radius: 0; border-left: 1px solid rgb(255 255 255 / .35); }
  .menu { position: absolute; top: calc(100% + 4px); left: 0; z-index: 10; display: grid; min-width: 11rem; padding: .3rem; background: var(--surface); border: 1px solid var(--line); border-radius: .5rem; box-shadow: 0 6px 20px rgb(0 0 0 / .25); }
  .menu button { justify-content: flex-start; text-align: left; border-color: transparent; background: transparent; }
  .menu button:hover:not(:disabled), .menu button:focus-visible { background: var(--hover); }
  .menu .sub { padding-left: 1.6rem; }
  hr { width: 100%; margin: .25rem 0; border: 0; border-top: 1px solid var(--line); }
</style>
