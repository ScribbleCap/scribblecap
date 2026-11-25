<script lang="ts">
  import { onMount } from 'svelte';
  let { choose }: { choose: (choice: 'save' | 'discard' | 'cancel') => void } = $props();
  import { trapTab } from './focus-trap';
  let first: HTMLButtonElement;
  let dialog: HTMLDivElement;
  onMount(() => first.focus());
  function onKey(event: KeyboardEvent) {
    if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); choose('cancel'); }
    else trapTab(event, dialog);
  }
</script>
<div class="scrim">
  <div class="prompt" bind:this={dialog} role="alertdialog" aria-modal="true" aria-labelledby="unsaved-title" aria-describedby="unsaved-text" tabindex="-1" onkeydown={onKey}>
    <h2 id="unsaved-title">Save your changes?</h2>
    <p id="unsaved-text">This image has unsaved changes. They will be lost if you continue without saving.</p>
    <div class="row">
      <button class="primary" bind:this={first} onclick={() => choose('save')}>Save…</button>
      <button onclick={() => choose('discard')}>Discard</button>
      <button onclick={() => choose('cancel')}>Cancel</button>
    </div>
  </div>
</div>
<style>
  .scrim { position: fixed; inset: 0; z-index: 20; display: grid; place-items: center; background: rgb(0 0 0 / .35); }
  .prompt { max-width: 26rem; padding: 1.2rem 1.4rem; background: var(--surface); color: var(--ink); border: 1px solid var(--line); border-radius: .7rem; box-shadow: 0 8px 30px rgb(0 0 0 / .3); }
  h2 { margin: 0 0 .5rem; font-size: 1rem; }
  p { margin: 0 0 1rem; color: var(--ink-2); font-size: .85rem; }
  .row { display: flex; gap: .5rem; justify-content: flex-end; }
</style>
