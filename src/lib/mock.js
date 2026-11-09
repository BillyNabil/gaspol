// Backend palsu untuk mode demo (browser). Data contoh, tidak menyentuh apa pun.
import { demo, emitDemo } from './tauri.js';
import { ymd } from './util.js';

const day = (n) => {
  const d = new Date();
  d.setDate(d.getDate() + n);
  return ymd(d);
};
const now = () => Math.floor(Date.now() / 1000);
const step = (text, done = false) => ({ text, done });

const tasks = [
  {
    id: 't1',
    title: 'Render teaser 15 detik',
    project: 'Klien Arunika',
    deadline: day(1),
    steps: [step('Buka project .aep versi final', true), step('Cek timing teks dengan beat musik'), step('Render preview H.264 1080p'), step('Kirim link preview ke klien')],
    focus_secs: 2820,
    snoozes: 1,
    created_at: now() - 90000,
    done: false,
  },
  {
    id: 't2',
    title: 'Benerin bug login token expired',
    project: 'Web app',
    deadline: day(0),
    steps: [step('Reproduksi bug di localhost'), step('Cek response endpoint /auth/refresh'), step('Tulis test untuk token expired')],
    focus_secs: 0,
    snoozes: 2,
    created_at: now() - 50000,
    done: false,
  },
  {
    id: 't3',
    title: 'Sculpt maskot untuk thumbnail',
    project: 'Studio',
    deadline: day(4),
    steps: [step('Buka file .blend blockout maskot'), step('Rapikan proporsi kepala dan tangan'), step('Render 3 pose untuk thumbnail')],
    focus_secs: 5400,
    snoozes: 0,
    created_at: now() - 200000,
    done: false,
  },
  {
    id: 't4',
    title: 'Rapikan aset map Roblox',
    project: null,
    deadline: null,
    steps: [step('Buka place file di Roblox Studio'), step('Kelompokkan model ke folder per zona')],
    focus_secs: 0,
    snoozes: 0,
    created_at: now() - 300000,
    done: false,
  },
  { id: 't5', title: 'Kirim invoice Oktober', steps: [], focus_secs: 900, created_at: now() - 400000, done: true, done_at: now() - 3600 },
  { id: 't6', title: 'Export logo variasi monokrom', steps: [], focus_secs: 1800, created_at: now() - 500000, done: true, done_at: now() - 7200 },
];

const settings = {
  ai_provider: 'custom',
  claude_model: 'claude-haiku-5-5',
  local_base_url: 'http://localhost:1234/v1',
  local_model: '',
  custom_base_url: 'https://openrouter.ai/api/v1',
  custom_model: '',
  custom_format: 'openai',
  hotkey: 'Ctrl+Space',
  autostart: true,
  report_hour: 22,
  grace_secs: 4,
  default_minutes: 50,
  default_mode: 'nudge',
  hud_top: 6,
  distractors: [
    { name: 'YouTube', patterns: ['youtube'] },
    { name: 'Discord', patterns: ['exe:discord', 'discord'] },
    { name: 'Telegram', patterns: ['exe:telegram', 'exe:unigram', 'telegram'] },
    { name: 'Steam', patterns: ['exe:steam', 'exe:steamwebhelper'] },
    { name: 'TikTok', patterns: ['tiktok'] },
    { name: 'X', patterns: ['/ x', 'twitter'] },
  ],
};

const live = {
  active: true,
  task_id: 't1',
  task_title: 'Render teaser 15 detik',
  step: 'Cek timing teks dengan beat musik',
  remaining: 1934,
  planned: 3000,
  focus_secs: 1012,
  distracted_secs: 54,
  distractions: 1,
  mode: 'nudge',
  distracted: false,
  current: 'After Effects',
};
const idle = { active: false };
let session = demo === 'live' || demo === 'pill' ? { ...live } : idle;

const h = (x) => x * 3600;
const m = (x) => x * 60;
const dayLog = {
  date: day(0),
  apps: {
    Blender: { secs: h(2) + m(10), distracting: false },
    'After Effects': { secs: h(1) + m(35), distracting: false },
    'VS Code': { secs: h(1) + m(42), distracting: false },
    Chrome: { secs: m(52), distracting: false },
    YouTube: { secs: m(48), distracting: true },
    Discord: { secs: m(21), distracting: true },
    Figma: { secs: m(18), distracting: false },
    Terminal: { secs: m(12), distracting: false },
  },
  sessions: [
    { task_title: 'Sculpt maskot untuk thumbnail', start: now() - h(9), focus_secs: m(47), distractions: 0, completed: true },
    { task_title: 'Render teaser 15 detik', start: now() - h(6), focus_secs: m(44), distractions: 2, completed: true },
    { task_title: 'Benerin bug login token expired', start: now() - h(3), focus_secs: m(18), distractions: 3, completed: false },
    { task_title: 'Render teaser 15 detik', start: now() - h(1), focus_secs: m(25), distractions: 1, completed: true },
  ],
  distractions: 7,
  snoozed: [],
};

const clone = (v) => JSON.parse(JSON.stringify(v));
const pushTasks = () => emitDemo('tasks', clone(tasks));
const find = (id) => tasks.find((t) => t.id === id);

export const mockApi = {
  state: async () => ({
    tasks: clone(tasks),
    settings: clone(settings),
    session: clone(session),
    has_claude_key: false,
    has_local_key: false,
    has_custom_key: true,
    hotkey_error: null,
    data_dir: '%APPDATA%\\dev.gaspol.desktop',
    days: [day(0)],
  }),
  capture: async (text) => {
    await new Promise((r) => setTimeout(r, 700));
    const task = {
      id: 't' + Date.now(),
      title: 'Render teaser klien Arunika',
      project: 'Klien Arunika',
      deadline: day(4),
      steps: [step('Buka project .aep versi final'), step('Cek timing teks dengan beat musik'), step('Render preview H.264 1080p'), step('Kirim link preview ke klien')],
      focus_secs: 0,
      snoozes: 0,
      created_at: now(),
      raw: text,
      ai: true,
      done: false,
    };
    tasks.push(task);
    pushTasks();
    return { task: clone(task), warning: null };
  },
  updateTask: async (t) => (Object.assign(find(t.id), t), pushTasks()),
  remove: async (id) => (tasks.splice(tasks.findIndex((t) => t.id === id), 1), pushTasks()),
  toggleStep: async (id, i) => ((find(id).steps[i].done = !find(id).steps[i].done), pushTasks()),
  complete: async (id, done) => ((find(id).done = done), (find(id).done_at = now()), pushTasks()),
  regen: async () => {},
  start: async (taskId) => {
    const t = find(taskId);
    session = { ...live, task_id: taskId, task_title: t?.title ?? 'Sesi fokus bebas', step: t?.steps.find((s) => !s.done)?.text ?? null, remaining: 3000, focus_secs: 0, distractions: 0 };
    emitDemo('session', clone(session));
  },
  stop: async () => ((session = idle), emitDemo('session', idle)),
  day: async () => clone(dayLog),
  saveSettings: async () => {},
  setSecret: async () => {},
  testAi: async () => 'OK dalam 1.1 dtk → "Edit video teaser klien A" · langkah 1: Buka project .aep versi final',
  localModels: async () => ['qwen/qwen3.5-9b', 'z-ai/glm-4.6', 'deepseek/deepseek-chat'],
  hud: async () => {},
  openMain: async () => {},
};

/** Pesan HUD awal untuk skenario demo. */
export const demoHud = {
  nudge: { kind: 'nudge', strict: false, label: 'YouTube', title: 'Render teaser 15 detik', step: 'Cek timing teks dengan beat musik', task_id: 't1' },
  done: { kind: 'done', task_id: 't1', title: 'Render teaser 15 detik', focus: 2940, distracted: 60, distractions: 1 },
  remind: { kind: 'remind', task_id: 't2', title: 'Benerin bug login token expired', when: 'hari ini', step: 'Reproduksi bug di localhost', snoozes: 2 },
  report: { kind: 'report', total: 31000, distracted: 4140, focus: 8040, distractions: 7, top: 'Blender' },
}[demo];

export const demoLive = live;
