export const reduced =
  typeof matchMedia !== 'undefined' && matchMedia('(prefers-reduced-motion: reduce)').matches;

/** Durasi animasi, 0 kalau user minta reduced motion. */
export const ms = (n) => (reduced ? 0 : n);

export function fmtClock(secs) {
  secs = Math.max(0, Math.floor(secs ?? 0));
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = secs % 60;
  const pad = (n) => String(n).padStart(2, '0');
  return h ? `${h}:${pad(m)}:${pad(s)}` : `${pad(m)}:${pad(s)}`;
}

export function fmtDur(secs) {
  const m = Math.round((secs ?? 0) / 60);
  if (m < 1) return secs > 0 ? '<1m' : '0m';
  const h = Math.floor(m / 60);
  return h ? `${h}j ${m % 60}m` : `${m}m`;
}

const HARI = ['Min', 'Sen', 'Sel', 'Rab', 'Kam', 'Jum', 'Sab'];
const BULAN = ['Jan', 'Feb', 'Mar', 'Apr', 'Mei', 'Jun', 'Jul', 'Agu', 'Sep', 'Okt', 'Nov', 'Des'];

export function ymd(d) {
  const pad = (n) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

export const todayStr = () => ymd(new Date());

export function parseYmd(s) {
  const [y, m, d] = s.split('-').map(Number);
  return new Date(y, m - 1, d);
}

export function daysUntil(deadline) {
  if (!deadline) return null;
  const t = new Date();
  t.setHours(0, 0, 0, 0);
  return Math.round((parseYmd(deadline) - t) / 86400000);
}

/** Label + nada warna untuk chip deadline. */
export function due(deadline) {
  const n = daysUntil(deadline);
  if (n === null) return null;
  const d = parseYmd(deadline);
  if (n < 0) return { label: `telat ${-n}h`, tone: 'danger' };
  if (n === 0) return { label: 'hari ini', tone: 'warn' };
  if (n === 1) return { label: 'besok', tone: 'warn' };
  if (n < 7) return { label: HARI[d.getDay()], tone: 'muted' };
  return { label: `${d.getDate()} ${BULAN[d.getMonth()]}`, tone: 'muted' };
}

export function prettyDate(s) {
  const d = parseYmd(s);
  const n = daysUntil(s);
  if (n === 0) return 'Hari ini';
  if (n === -1) return 'Kemarin';
  return `${HARI[d.getDay()]}, ${d.getDate()} ${BULAN[d.getMonth()]}`;
}

export function sortTasks(tasks) {
  return [...tasks].sort((a, b) => {
    const da = daysUntil(a.deadline);
    const db = daysUntil(b.deadline);
    if (da !== db) {
      if (da === null) return 1;
      if (db === null) return -1;
      return da - db;
    }
    return b.created_at - a.created_at;
  });
}

export const nextStep = (t) => t?.steps?.find((s) => !s.done)?.text ?? null;

export const MODES = [
  { id: 'passive', name: 'Pasif', hint: 'Cuma mencatat' },
  { id: 'nudge', name: 'Negur', hint: 'Muncul kalau kamu ngelantur' },
  { id: 'strict', name: 'Galak', hint: 'Jendela pengganggu di-minimize' },
];

export const modeName = (id) => MODES.find((m) => m.id === id)?.name ?? id;
