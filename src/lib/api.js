import { invoke } from '@tauri-apps/api/core';
import { isTauri } from './tauri.js';
import { mockApi } from './mock.js';

const tauriApi = {
  state: () => invoke('get_state'),
  capture: (text) => invoke('capture_task', { text }),
  captureImage: (b64, mime, note = '') => invoke('capture_image', { b64, mime, note }),
  transcribe: (b64, mime) => invoke('transcribe', { b64, mime }),
  updateTask: (task) => invoke('update_task', { task }),
  remove: (id) => invoke('delete_task', { id }),
  toggleStep: (id, index) => invoke('toggle_step', { id, index }),
  complete: (id, done) => invoke('complete_task', { id, done }),
  regen: (id) => invoke('regenerate_steps', { id }),
  start: (taskId = null, minutes = null, mode = null) => invoke('start_session', { taskId, minutes, mode }),
  stop: () => invoke('stop_session'),
  day: (date) => invoke('get_day', { date }),
  saveSettings: (settings) => invoke('save_settings', { settings }),
  setSecret: (kind, value) => invoke('set_secret', { kind, value }),
  testAi: () => invoke('test_ai'),
  localModels: (baseUrl, kind = 'local') => invoke('local_models', { baseUrl, kind }),
  hud: (action, taskId = null, value = null) => invoke('hud_action', { action, taskId, value }),
  openMain: (tab = null) => invoke('open_main', { tab }),
  openPath: (path) => invoke('open_path', { path }),
  reveal: (path) => invoke('reveal_path', { path }),
  runPreset: (id) => invoke('run_preset', { id }),
  closeDistractors: () => invoke('close_distractors'),
  renderAdd: (file, comp = null, frames = null, project = null) => invoke('render_add', { file, comp, frames, project }),
  renderCancel: (id) => invoke('render_cancel', { id }),
  renderRemove: (id) => invoke('render_remove', { id }),
  renderRetry: (id) => invoke('render_retry', { id }),
  renderClear: () => invoke('render_clear'),
  renderState: () => invoke('render_state'),
  week: (end) => invoke('get_week', { end }),
  stats: () => invoke('get_stats'),
  plan: () => invoke('get_plan'),
  clients: (from, to) => invoke('get_clients', { from, to }),
  exportClients: (from, to) => invoke('export_clients', { from, to }),
  invoice: (project, from, to) => invoke('make_invoice', { project, from, to }),
  clientUpdate: (project) => invoke('client_update', { project }),
  exportIcs: () => invoke('export_ics'),
  refreshCalendar: () => invoke('refresh_calendar'),
  testWebhook: (url) => invoke('test_webhook', { url }),
  sendDailyNow: () => invoke('send_daily_now'),
  telegramPair: () => invoke('telegram_pair'),
  telegramUnpair: () => invoke('telegram_unpair'),
  gitPoll: () => invoke('git_poll_now'),
  pickFile: async (filters) => {
    const { open } = await import('@tauri-apps/plugin-dialog');
    return open({ multiple: true, filters });
  },
  pickFolder: async () => {
    const { open } = await import('@tauri-apps/plugin-dialog');
    return open({ directory: true });
  },
  checkUpdate: async () => {
    const { check } = await import('@tauri-apps/plugin-updater');
    return check();
  },
  relaunch: async () => {
    const { relaunch } = await import('@tauri-apps/plugin-process');
    return relaunch();
  },
};

export const api = isTauri ? tauriApi : mockApi;
