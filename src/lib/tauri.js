// Jembatan tipis ke Tauri. Di luar Tauri (browser biasa) app jalan dalam mode demo
// dengan data contoh — dipakai untuk screenshot & preview UI.
import { getCurrentWindow } from '@tauri-apps/api/window';
import { listen as tauriListen } from '@tauri-apps/api/event';

export const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

const params = new URLSearchParams(typeof location !== 'undefined' ? location.search : '');
/** Skenario demo: ?demo=live | done | nudge | pill | remind | report */
export const demo = isTauri ? null : params.get('demo') || 'idle';

const bus = new Map();
export function emitDemo(event, payload) {
  (bus.get(event) ?? []).forEach((cb) => cb({ payload }));
}

const demoWin = {
  label: params.get('win') || 'main',
  minimize: async () => {},
  hide: async () => {},
  close: async () => {},
  onFocusChanged: async () => () => {},
};

export const currentWindow = () => (isTauri ? getCurrentWindow() : demoWin);

export function listen(event, cb) {
  if (isTauri) return tauriListen(event, cb);
  if (!bus.has(event)) bus.set(event, []);
  bus.get(event).push(cb);
  return Promise.resolve(() => bus.set(event, bus.get(event).filter((f) => f !== cb)));
}
