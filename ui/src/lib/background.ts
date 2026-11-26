import { invoke } from '@tauri-apps/api/core';
import { decodeImagePacket } from './image-packet';
import type { CaptureMode,CaptureSettings } from './capture';
export type BackgroundAction = {kind:'capture';mode:CaptureMode;delay:number} | {kind:'settings'|'quit'|'show'} | {kind:'error';message:string};
export interface BackgroundStatus {trayAvailable:boolean;hotkeyAvailable:boolean;hotkeyError:string|null;hotkeyDescription?:string|null;trayNote:string|null;platform:string}
export const backgroundStatus = () => invoke<BackgroundStatus>('shell_background_status');
export const backgroundReady = () => invoke<void>('shell_background_ready');
export const showEditor = () => invoke<void>('shell_show_editor');
export const finishQuit = () => invoke<void>('shell_finish_quit');
export function setBackgroundSettings(hotkey:string,keepRunning:boolean,hideDockIcon:boolean):Promise<CaptureSettings> {return invoke('shell_set_background_settings',{hotkey,keepRunning,hideDockIcon});}
export async function backgroundCapture(mode:CaptureMode,delay:number){
  const response=await invoke<ArrayBuffer|number[]>('shell_background_capture',{mode,delay});
  return decodeImagePacket(response instanceof ArrayBuffer?response:new Uint8Array(response).buffer);
}
