<script lang="ts">
  import { onMount } from 'svelte';
  import { chooseCaptureFolder, getSettings, setAutoSave, type CaptureSettings } from './capture';
  let { close }: { close: () => void } = $props();
  let settings = $state<CaptureSettings>({ captureFolder: null, autoSave: false });
  let error = $state('');
  let closeButton: HTMLButtonElement;
  const fail = (cause: unknown) => { error = cause instanceof Error ? cause.message : String(cause); };
  onMount(() => {
    closeButton.focus();
    getSettings().then(next => { settings = next; }).catch(fail);
  });
  async function change() {
    error = '';
    try { settings = await chooseCaptureFolder(); } catch (cause) { fail(cause); }
  }
  async function toggle(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    error = '';
    try { settings = await setAutoSave(input.checked); } catch (cause) { fail(cause); }
    input.checked = settings.autoSave;
  }
  function onKey(event: KeyboardEvent) {
    if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); close(); }
  }
</script>
<div class="scrim">
  <div class="panel" role="dialog" aria-modal="true" aria-label="Settings" tabindex="-1" onkeydown={onKey}>
    <h2>Settings</h2>
    {#if error}<p class="problem" role="alert">{error}</p>{/if}
    <div class="setting">
      <div><strong id="folder-label">Capture save folder</strong><span class="folder" title={settings.captureFolder ?? undefined}>{settings.captureFolder ?? 'Not chosen yet'}</span></div>
      <button aria-describedby="folder-label" onclick={change}>Change…</button>
    </div>
    <div class="setting">
      <div><strong id="auto-save-label">Auto-save captures</strong><span id="auto-save-hint">Save each new capture to the folder above as soon as it is taken.</span></div>
      <input type="checkbox" role="switch" aria-labelledby="auto-save-label" aria-describedby="auto-save-hint" checked={settings.autoSave} onchange={toggle} />
    </div>
    <div class="row"><button bind:this={closeButton} onclick={close}>Close</button></div>
  </div>
</div>
<style>
  .scrim { position: fixed; inset: 0; z-index: 20; display: grid; place-items: center; background: rgb(0 0 0 / .35); }
  .panel { width: min(30rem, calc(100vw - 2rem)); padding: 1.2rem 1.4rem; background: var(--surface); color: var(--ink); border: 1px solid var(--line); border-radius: .7rem; box-shadow: 0 8px 30px rgb(0 0 0 / .3); }
  h2 { margin: 0 0 .8rem; font-size: 1rem; }
  .setting { display: flex; align-items: center; justify-content: space-between; gap: 1rem; padding: .6rem 0; border-top: 1px solid var(--line); }
  .setting div { display: grid; gap: .15rem; min-width: 0; }
  .setting span { color: var(--ink-2); font-size: .8rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .setting #auto-save-hint { white-space: normal; }
  .problem { margin: 0 0 .6rem; padding: .4rem .6rem; color: var(--error-ink); background: var(--error-bg); border-radius: .4rem; font-size: .8rem; }
  .row { display: flex; justify-content: flex-end; margin-top: .6rem; }
</style>
