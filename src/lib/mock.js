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
    auto_secs: 1500,
    est_minutes: 240,
    allowed: ['exe:afterfx', 'exe:explorer', 'figma'],
    template: 'Video klien',
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
  hud_monitor: 0,
  selection_hotkey: 'Ctrl+Shift+Space',
  allow_keywords: ['tutorial', 'course', 'blender', 'after effects', 'roblox studio', 'docs', 'github'],
  intent_mode: true,
  flow_protect: true,
  break_every_min: 120,
  hard_block: false,
  block_sites: ['youtube.com', 'www.youtube.com', 'tiktok.com', 'instagram.com', 'x.com', 'reddit.com'],
  music: '',
  presets: [
    { id: 'motion', name: 'Motion (AE)', open: ['afterfx', 'D:\\Klien\\Arunika'], close_distractors: true, minutes: 50, mode: 'nudge', project: 'Klien Arunika' },
    { id: 'coding', name: 'Coding', open: ['code', 'https://github.com'], close_distractors: true, minutes: 50, mode: 'nudge', project: 'Web app' },
    { id: 'skripsi', name: 'Skripsi', open: ['winword'], close_distractors: true, minutes: 90, mode: 'strict', project: null },
  ],
  templates: [
    { name: 'Video klien', keywords: ['teaser', 'video', 'reels', 'motion'], steps: ['Buka brief & kumpulkan aset klien', 'Susun urutan shot / storyboard', 'Bikin animatic kasar', 'Animasi & polish', 'Render preview & kirim ke klien'], est_minutes: 360 },
    { name: 'Bug fix', keywords: ['bug', 'error', 'fix'], steps: ['Reproduksi bug di lokal', 'Cari penyebab', 'Tulis test', 'Perbaiki', 'Commit & deploy'], est_minutes: 120 },
  ],
  blender_path: '',
  aerender_path: '',
  after_queue: 'none',
  render_break: true,
  organize_exports: true,
  watch_dirs: ['D:\\Render\\out'],
  export_root: 'D:\\Klien',
  auto_track: true,
  git_repos: [{ path: 'D:\\code\\web-app', project: 'Web app' }],
  ics_url: '',
  webhook_url: '',
  telegram_chat_id: null,
  telegram_pair_code: '',
  stt_base_url: 'https://api.groq.com/openai/v1',
  stt_model: 'whisper-large-v3-turbo',
  plan_hour: 9,
  rates: { 'Klien Arunika': 150000, 'Web app': 200000 },
  currency: 'Rp',
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
    has_telegram_key: false,
    has_stt_key: true,
    telegram_ok: false,
    telegram_error: null,
    cal_error: null,
    cal_count: 3,
    stats: clone(stats),
    render: clone(renderView),
    monitors: ['Monitor 1 · 1920×1080', 'Monitor 2 · 2560×1440'],
    version: '0.2.0',
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
  openPath: async () => {},
  reveal: async () => {},
  captureImage: async () => mockApi.capture('revisi dari screenshot'),
  transcribe: async () => 'render teaser klien Arunika sebelum jumat',
  runPreset: async () => mockApi.start('t1', 50, 'nudge'),
  closeDistractors: async () => 2,
  renderAdd: async () => {},
  renderCancel: async () => {},
  renderRemove: async () => {},
  renderRetry: async () => {},
  renderClear: async () => {},
  renderState: async () => clone(renderView),
  week: async () => clone(week),
  stats: async () => clone(stats),
  plan: async () => clone(plan),
  clients: async () => clone(clientRows),
  exportClients: async () => 'C:\\Users\\kamu\\Downloads\\gaspol-klien.csv',
  invoice: async () => 'C:\\Users\\kamu\\Downloads\\INV.html',
  clientUpdate: async () => 'Halo! Update progres Klien Arunika:\n• Render teaser 15 detik — 1/4 tahap\nBerikutnya: cek timing teks dengan beat musik. Nanti saya kabari lagi.',
  exportIcs: async () => '%APPDATA%\\dev.gaspol.desktop\\gaspol.ics',
  refreshCalendar: async () => 3,
  testWebhook: async () => {},
  sendDailyNow: async () => {},
  telegramPair: async () => '482913',
  telegramUnpair: async () => {},
  gitPoll: async () => 4,
  pickFile: async () => null,
  pickFolder: async () => null,
  checkUpdate: async () => null,
  relaunch: async () => {},
};

const stats = { xp: 2140, level: 6, level_from: 1500, level_to: 2160, streak: 4, ratio: 1.6, samples: 7 };

const renderView = {
  jobs: [
    { id: 'r1', kind: 'ae', file: 'D:\\Klien\\Arunika\\teaser_final.aep', comp: 'MAIN 15s', frames: '', project: 'Klien Arunika', status: 'running', progress: 0.42, frame: 189, total: 450, started: now() - 260, ended: null, message: '', output: null },
    { id: 'r2', kind: 'blender', file: 'D:\\3D\\maskot\\maskot_pose.blend', comp: '', frames: '', project: 'Studio', status: 'queued', progress: 0, frame: null, total: null, started: null, ended: null, message: '', output: null },
    { id: 'r3', kind: 'blender', file: 'D:\\3D\\maskot\\thumbnail.blend', comp: '', frames: '1', project: 'Studio', status: 'done', progress: 1, frame: 1, total: 1, started: now() - 4000, ended: now() - 3700, message: '', output: 'D:\\Klien\\Studio\\2026-11-09_thumbnail_v01.png' },
  ],
  external: null,
  external_secs: 0,
  blender: 'C:\\Program Files\\Blender Foundation\\Blender 4.5\\blender.exe',
  aerender: 'C:\\Program Files\\Adobe\\Adobe After Effects 2026\\Support Files\\aerender.exe',
};

const hourly = (peak) => Array.from({ length: 24 }, (_, h) => Math.max(0, Math.round((1 - Math.abs(h - peak) / 5) * 3000 + (h > 8 && h < 23 ? 400 : 0))));
const week = {
  days: [6, 5, 4, 3, 2, 1, 0].map((n, i) => ({ date: day(-n), focus: [9800, 12400, 4200, 15100, 11000, 2600, 13400][i], distracted: [2100, 1600, 3900, 900, 1500, 4200, 1400][i], total: 30000, sessions: 3, commits: [2, 5, 0, 7, 3, 0, 4][i], renders: [1, 0, 2, 0, 1, 0, 1][i] })),
  heat: [20, 21, 14, 21, 20, 15, 21].map((p) => hourly(p)),
  heat_dis: [0, 0, 0, 0, 0, 0, 0].map(() => Array.from({ length: 24 }, (_, h) => (h === 13 || h === 16 ? 900 : 0))),
  best: '20:00–23:00',
  focus: 68500,
  distracted: 15600,
  prev_focus: 59000,
  stats,
};

const plan = {
  tasks: [
    { id: 't2', title: 'Benerin bug login token expired', step: 'Reproduksi bug di localhost', deadline: day(0), need_min: 190 },
    { id: 't1', title: 'Render teaser 15 detik', step: 'Cek timing teks dengan beat musik', deadline: day(1), need_min: 280 },
    { id: 't3', title: 'Sculpt maskot untuk thumbnail', step: 'Buka file .blend blockout maskot', deadline: day(4), need_min: null },
  ],
  free: [],
  free_text: '13:00–15:30, 19:00–22:00',
  events: [
    { title: 'Bimbingan skripsi', start: now() + 3600, end: now() + 7200, all_day: false },
    { title: 'Call klien Arunika', start: now() + 5 * 3600, end: now() + 5.5 * 3600, all_day: false },
  ],
};

const clientRows = [
  { project: 'Klien Arunika', secs: 41200, rate: 150000, amount: 1716667, sessions: 14, done: ['Export logo variasi monokrom', 'Storyboard teaser'] },
  { project: 'Web app', secs: 28800, rate: 200000, amount: 1600000, sessions: 9, done: ['Kirim invoice Oktober'] },
  { project: 'Studio', secs: 19800, rate: 0, amount: 0, sessions: 6, done: [] },
];

/** Pesan HUD awal untuk skenario demo. */
export const demoHud = {
  nudge: { kind: 'nudge', strict: false, label: 'YouTube', title: 'Render teaser 15 detik', step: 'Cek timing teks dengan beat musik', task_id: 't1' },
  done: { kind: 'done', task_id: 't1', title: 'Render teaser 15 detik', focus: 2940, distracted: 60, distractions: 1 },
  remind: { kind: 'remind', task_id: 't2', title: 'Benerin bug login token expired', when: 'hari ini', step: 'Reproduksi bug di localhost', snoozes: 2 },
  report: { kind: 'report', total: 31000, distracted: 4140, focus: 8040, distractions: 7, top: 'Blender' },
  intent: { kind: 'intent', label: 'YouTube', title: 'Render teaser 15 detik', task_id: 't1' },
  offtask: { kind: 'offtask', label: 'Spotify', title: 'Render teaser 15 detik', task_id: 't1' },
  break: { kind: 'break', mins: 120 },
  render: { kind: 'render', label: 'After Effects', secs: 340 },
  render_done: { kind: 'render_done', ok: true, name: 'teaser_final.aep', secs: 1260, output: 'D:\\Klien\\Arunika\\2026-11-09_teaser_final_v03.mp4' },
  power: { kind: 'power', action: 'shutdown' },
  plan: { kind: 'plan', tasks: [{ id: 't2', title: 'Benerin bug login token expired', step: 'Reproduksi bug di localhost' }, { id: 't1', title: 'Render teaser 15 detik', step: 'Cek timing teks dengan beat musik' }, { id: 't3', title: 'Sculpt maskot untuk thumbnail', step: 'Buka file .blend blockout maskot' }], free: '13:00–15:30, 19:00–22:00' },
}[demo];

export const demoRender = renderView;

export const demoLive = live;
