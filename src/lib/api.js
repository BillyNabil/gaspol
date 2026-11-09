import { invoke } from '@tauri-apps/api/core';
import { isTauri } from './tauri.js';
import { mockApi } from './mock.js';

const tauriApi = {
  state: () => invoke('get_state'),
  capture: (text) => invoke('capture_task', { text }),
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
  hud: (action, taskId = null) => invoke('hud_action', { action, taskId }),
  openMain: (tab = null) => invoke('open_main', { tab }),
};

export const api = isTauri ? tauriApi : mockApi;
