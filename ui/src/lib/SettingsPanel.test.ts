import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { beforeEach, expect, it, vi } from 'vitest';
import SettingsPanel from './SettingsPanel.svelte';
import { backgroundStatus,setBackgroundSettings } from './background';
import { chooseCaptureFolder, getSettings, setAutoSave } from './capture';

vi.mock('@tauri-apps/api/event',()=>({listen:vi.fn().mockResolvedValue(vi.fn())}));
vi.mock('./background',()=>({backgroundStatus:vi.fn().mockResolvedValue({trayAvailable:true,hotkeyAvailable:true,hotkeyError:null,trayNote:null,platform:'macos'}),setBackgroundSettings:vi.fn()}));
vi.mock('./capture', () => ({ getSettings: vi.fn(), setAutoSave: vi.fn(), chooseCaptureFolder: vi.fn() }));
beforeEach(() => {
  vi.mocked(getSettings).mockReset().mockResolvedValue({ captureFolder: null, autoSave: false });
  vi.mocked(setAutoSave).mockReset();
  vi.mocked(chooseCaptureFolder).mockReset();
});

it('shows auto-save off and no folder by default', async () => {
  render(SettingsPanel, { close: vi.fn() });
  await waitFor(() => expect(screen.getByText('Not chosen yet')).toBeTruthy());
  expect((screen.getByRole('switch', { name: 'Auto-save captures' }) as HTMLInputElement).checked).toBe(false);
  expect(screen.getByText('Capture save folder')).toBeTruthy();
});
it('changes the folder through the picker', async () => {
  vi.mocked(chooseCaptureFolder).mockResolvedValue({ captureFolder: '/home/me/Shots', autoSave: false });
  render(SettingsPanel, { close: vi.fn() });
  await waitFor(() => expect(screen.getByText('Not chosen yet')).toBeTruthy());
  await fireEvent.click(screen.getByRole('button', { name: 'Change…' }));
  await waitFor(() => expect(screen.getByText('/home/me/Shots')).toBeTruthy());
  expect(chooseCaptureFolder).toHaveBeenCalledOnce();
});
it('turns auto-save on and reports a failure', async () => {
  vi.mocked(setAutoSave).mockResolvedValueOnce({ captureFolder: null, autoSave: true }).mockRejectedValueOnce(new Error('The settings are unavailable.'));
  render(SettingsPanel, { close: vi.fn() });
  const toggle = await screen.findByRole('switch', { name: 'Auto-save captures' }) as HTMLInputElement;
  await fireEvent.click(toggle);
  await waitFor(() => expect(setAutoSave).toHaveBeenCalledWith(true));
  await waitFor(() => expect(toggle.checked).toBe(true));
  await fireEvent.click(toggle);
  await waitFor(() => expect(screen.getByRole('alert').textContent).toContain('The settings are unavailable.'));
  expect(toggle.checked).toBe(true);
});
it('closes with the Close button and Escape', async () => {
  const close = vi.fn();
  render(SettingsPanel, { close });
  await fireEvent.click(screen.getByRole('button', { name: 'Close' }));
  await fireEvent.keyDown(screen.getByRole('dialog'), { key: 'Escape' });
  expect(close).toHaveBeenCalledTimes(2);
});

it('reports shortcut registration errors and keeps the settings panel usable',async()=>{
  vi.mocked(setBackgroundSettings).mockRejectedValueOnce(new Error('Shortcut already in use'));
  render(SettingsPanel,{close:vi.fn()});
  await waitFor(()=>expect((screen.getByLabelText('Global region shortcut') as HTMLInputElement).disabled).toBe(false));
  await fireEvent.input(screen.getByLabelText('Global region shortcut'),{target:{value:'Ctrl+Shift+Digit6'}});
  await fireEvent.click(screen.getByRole('button',{name:'Apply background settings'}));
  await waitFor(()=>expect(screen.getByRole('alert').textContent).toContain('already in use'));
});
it('disables unsupported Wayland shortcut configuration with its reason',async()=>{
  vi.mocked(backgroundStatus).mockResolvedValueOnce({trayAvailable:false,hotkeyAvailable:false,hotkeyError:'This desktop has no GlobalShortcuts portal',trayNote:'No tray',platform:'linux'});
  render(SettingsPanel,{close:vi.fn()});
  await waitFor(()=>expect(screen.getByText(/no GlobalShortcuts portal/)).toBeTruthy());
  expect((screen.getByLabelText('Global region shortcut') as HTMLInputElement).disabled).toBe(true);
  expect((screen.getByLabelText('Keep running in the tray/menu bar when the window is closed') as HTMLInputElement).checked).toBe(false);
  expect(screen.queryByLabelText('Hide Dock icon')).toBeNull();
});
