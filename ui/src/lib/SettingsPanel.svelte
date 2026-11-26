<script lang="ts">
  import { listen } from '@tauri-apps/api/event';
  import { backgroundStatus,setBackgroundSettings,type BackgroundStatus } from './background';
  import { onMount } from 'svelte';
  import { chooseCaptureFolder, getSettings, setAutoSave, type CaptureSettings } from './capture';
  let { close }: { close: () => void } = $props();
  let settings = $state<CaptureSettings>({ captureFolder: null, autoSave: false });
  let error = $state('');
  let status = $state<BackgroundStatus>({trayAvailable:false,hotkeyAvailable:false,hotkeyError:null,trayNote:null,platform:''});
  let hotkey = $state('CmdOrCtrl+Shift+Digit2');
  let keepRunning = $state(true), hideDock = $state(false);
  let saving = $state(false);
  import { trapTab } from './focus-trap';
  let closeButton: HTMLButtonElement;
  let dialog: HTMLDivElement;
  const fail = (cause: unknown) => { error = cause instanceof Error ? cause.message : String(cause); };
  onMount(() => {
    closeButton.focus();
    getSettings().then(next => { settings = next;hotkey=next.captureHotkey ?? hotkey;keepRunning=next.keepRunning ?? true;hideDock=next.hideDockIcon ?? false; }).catch(fail);
    backgroundStatus().then(next => status=next).catch(fail);
    let mounted=true;let stop:(()=>void)|undefined;
    void listen<BackgroundStatus>('background-status',event=>{status=event.payload;}).then(unlisten=>{if(mounted)stop=unlisten;else unlisten();}).catch(fail);
    return()=>{mounted=false;stop?.();};
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
  async function saveBackground() {
    saving=true;error='';
    try {settings=await setBackgroundSettings(hotkey,keepRunning,hideDock);status=await backgroundStatus();}
    catch(cause){fail(cause);}finally{saving=false;}
  }
  function onKey(event: KeyboardEvent) {
    if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); close(); }
    else trapTab(event, dialog);
  }
</script>
<div class="scrim">
  <div class="panel" bind:this={dialog} role="dialog" aria-modal="true" aria-label="Settings" tabindex="-1" onkeydown={onKey}>
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
    <form onsubmit={(event)=>{event.preventDefault();void saveBackground();}}>
      <div class="setting"><label for="capture-hotkey"><strong>Global region shortcut</strong></label><input id="capture-hotkey" bind:value={hotkey} disabled={!status.hotkeyAvailable || saving} /></div>
      <p class="help">Use modifiers and a key, for example CmdOrCtrl+Shift+Digit2. The shortcut works while another app is focused.</p>
      {#if status.hotkeyDescription}<p class="help">Registered shortcut: {status.hotkeyDescription}</p>{/if}
      {#if status.hotkeyError}<p class="problem" role="status">{status.hotkeyError}</p>{/if}
      <div class="setting"><label for="keep-running">Keep running in the tray/menu bar when the window is closed</label><input id="keep-running" type="checkbox" checked={keepRunning && status.trayAvailable} disabled={!status.trayAvailable || saving} onchange={(event)=>keepRunning=event.currentTarget.checked} /></div>
      {#if status.trayNote}<p class="help">{status.trayNote}</p>{/if}
      {#if status.platform==='macos'}<div class="setting"><label for="hide-dock">Hide Dock icon</label><input id="hide-dock" type="checkbox" bind:checked={hideDock} disabled={!status.trayAvailable || saving} /></div>{/if}
      <button type="submit" disabled={saving}>Apply background settings</button>
    </form>
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
  .help { font-size:.8rem;color:var(--ink-2);white-space:normal; }
  .panel {max-height:90vh;overflow:auto;}
  .row { display: flex; justify-content: flex-end; margin-top: .6rem; }
</style>
