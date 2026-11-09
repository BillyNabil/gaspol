<script>
  import { onMount } from 'svelte';
  import { fly } from 'svelte/transition';
  import { quintOut } from 'svelte/easing';
  import { api } from '../lib/api.js';
  import { fmtDur, todayStr, prettyDate, parseYmd, ymd, ms } from '../lib/util.js';
  import ReportWeek from './ReportWeek.svelte';
  import ReportClients from './ReportClients.svelte';

  let { st, flash } = $props();

  const VIEWS = [
    { id: 'day', name: 'Hari' },
    { id: 'week', name: 'Minggu' },
    { id: 'clients', name: 'Klien' },
  ];
  let view = $state('day');
  const vIdx = $derived(VIEWS.findIndex((v) => v.id === view));
  let sharing = $state(false);

  async function share() {
    sharing = true;
    try {
      await api.sendDailyNow();
      flash?.('Ringkasan dikirim ke Discord/Telegram');
    } finally {
      sharing = false;
    }
  }

  let date = $state(todayStr());
  let day = $state(null);
  let ready = $state(false);

  async function load() {
    const d = date;
    const v = await api.day(d);
    if (d === date) day = v;
  }

  $effect(() => {
    date;
    ready = false;
    load().then(() => requestAnimationFrame(() => (ready = true)));
  });

  onMount(() => {
    const t = setInterval(() => date === todayStr() && load(), 15000);
    return () => clearInterval(t);
  });

  function shift(n) {
    const d = parseYmd(date);
    d.setDate(d.getDate() + n);
    const next = ymd(d);
    if (next <= todayStr()) date = next;
  }

  const apps = $derived(
    day
      ? Object.entries(day.apps)
          .map(([name, a]) => ({ name, ...a }))
          .filter((a) => a.secs >= 30)
          .sort((a, b) => b.secs - a.secs)
          .slice(0, 9)
      : [],
  );
  const max = $derived(apps[0]?.secs || 1);
  const total = $derived(day ? Object.values(day.apps).reduce((n, a) => n + a.secs, 0) : 0);
  const distracted = $derived(day ? Object.values(day.apps).filter((a) => a.distracting).reduce((n, a) => n + a.secs, 0) : 0);
  const focus = $derived(day ? day.sessions.reduce((n, s) => n + s.focus_secs, 0) + Object.values(day.task_secs ?? {}).reduce((n, v) => n + v, 0) : 0);
  const sessions = $derived(day ? [...day.sessions].reverse() : []);
  const snoozed = $derived(st.tasks.filter((t) => !t.done && t.snoozes > 0).sort((a, b) => b.snoozes - a.snoozes).slice(0, 5));

  const verdict = $derived.by(() => {
    if (total < 600) return 'Belum banyak data hari ini. Laptop nyala, Gaspol nyatet diam-diam.';
    const ratio = distracted / total;
    if (focus >= 3 * 3600 && ratio < 0.15) return 'Hari yang solid. Pertahankan ritme ini.';
    if (ratio >= 0.35) return `Sepertiga lebih waktumu habis di pengganggu. Coba mode Galak besok.`;
    if (focus < 1800) return 'Belum ada sesi fokus yang berarti. Mulai satu 25 menit aja.';
    return 'Lumayan. Satu sesi fokus lagi besok bikin beda.';
  });

  const timeOf = (ts) => new Date(ts * 1000).toLocaleTimeString('id-ID', { hour: '2-digit', minute: '2-digit' });
</script>

<div class="wrap">
  <div class="switch-row">
    <div class="seg">
      <span class="thumb" style="width: calc((100% - 6px) / 3); transform: translateX({vIdx * 100}%)"></span>
      {#each VIEWS as v}
        <button class:on={view === v.id} onclick={() => (view = v.id)}>{v.name}</button>
      {/each}
    </div>
    {#if view === 'day' && (st.settings.webhook_url || st.settings.telegram_chat_id)}
      <button class="btn sm ghost" disabled={sharing} onclick={share}>Bagikan ringkasan</button>
    {/if}
  </div>

  {#if view === 'week'}
    <ReportWeek />
  {:else if view === 'clients'}
    <ReportClients {st} {flash} />
  {:else}
  <div class="head">
    <button class="nav" aria-label="Hari sebelumnya" onclick={() => shift(-1)}>‹</button>
    <h1 class="display">{prettyDate(date)}</h1>
    <button class="nav" aria-label="Hari berikutnya" disabled={date >= todayStr()} onclick={() => shift(1)}>›</button>
  </div>
  <p class="verdict">{verdict}</p>

  <div class="kpis">
    {#each [{ k: 'Fokus', v: fmtDur(focus), accent: true }, { k: 'Ngelantur', v: fmtDur(distracted), warn: distracted > 0 }, { k: 'Total aktif', v: fmtDur(total) }, { k: 'Kali kabur', v: `${day?.distractions ?? 0}×` }] as kpi, i}
      <div class="kpi" in:fly={{ y: 8, duration: ms(380), delay: ms(i * 50), easing: quintOut }}>
        <span>{kpi.k}</span>
        <b class="num display" class:accent={kpi.accent} class:warn={kpi.warn}>{kpi.v}</b>
      </div>
    {/each}
  </div>

  <section>
    <h3>Waktu per app</h3>
    {#if apps.length}
      <ul class="bars">
        {#each apps as a, i (a.name)}
          <li>
            <span class="name">{a.name}</span>
            <span class="track">
              <i
                class:warn={a.distracting}
                style="transform: scaleX({ready ? a.secs / max : 0}); transition-delay: {ms(i * 40)}ms"
              ></i>
            </span>
            <span class="val num">{fmtDur(a.secs)}</span>
          </li>
        {/each}
      </ul>
    {:else}
      <p class="empty">Belum ada aktivitas tercatat.</p>
    {/if}
  </section>

  <div class="cols">
    <section>
      <h3>Sesi fokus</h3>
      {#if sessions.length}
        <ul class="sessions">
          {#each sessions as s}
            <li>
              <span class="time num">{timeOf(s.start)}</span>
              <span class="what">{s.task_title}</span>
              <span class="num muted">{fmtDur(s.focus_secs)}{s.distractions ? ` · ${s.distractions}×` : ''}</span>
              <span class="chip {s.completed ? 'accent' : ''}">{s.completed ? 'tuntas' : 'berhenti'}</span>
            </li>
          {/each}
        </ul>
      {:else}
        <p class="empty">Belum ada sesi.</p>
      {/if}
    </section>
    <section>
      <h3>Paling sering ditunda</h3>
      {#if snoozed.length}
        <ul class="sessions">
          {#each snoozed as t}
            <li><span class="what">{t.title}</span><span class="chip warn">{t.snoozes}×</span></li>
          {/each}
        </ul>
      {:else}
        <p class="empty">Nggak ada yang ditunda. Mantap.</p>
      {/if}
    </section>
  </div>

  {#if day?.commits?.length}
    <section>
      <h3>Commit hari ini</h3>
      <ul class="sessions">
        {#each [...day.commits].reverse().slice(0, 8) as c}
          <li><span class="time num">{timeOf(c.ts)}</span><span class="what">{c.message}</span><span class="chip">{c.project}</span></li>
        {/each}
      </ul>
    </section>
  {/if}
  {/if}
</div>

<style>
  .switch-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 18px;
  }
  .wrap {
    max-width: 760px;
    margin: 0 auto;
    padding: 14px 28px 48px;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  h1 {
    margin: 0;
    font-size: 26px;
    font-weight: 650;
    letter-spacing: -0.02em;
    min-width: 200px;
    text-align: center;
  }
  .nav {
    width: 32px;
    height: 32px;
    border-radius: 50%;
    font-size: 20px;
    line-height: 1;
    color: var(--muted);
    transition: background-color 0.2s var(--ease);
  }
  .nav:hover:not(:disabled) {
    background: var(--surface-2);
    color: var(--text);
  }
  .nav:disabled {
    opacity: 0.25;
    cursor: default;
  }
  .head {
    justify-content: center;
  }
  .verdict {
    margin: 6px 0 22px;
    text-align: center;
    color: var(--muted);
  }
  .kpis {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 10px;
  }
  .kpi {
    padding: 14px 16px;
    border-radius: var(--r);
    background: var(--surface);
    border: 1px solid var(--line);
  }
  .kpi span {
    font-size: 12px;
    color: var(--muted);
  }
  .kpi b {
    display: block;
    margin-top: 4px;
    font-size: 24px;
    font-weight: 600;
    letter-spacing: -0.02em;
  }
  .kpi b.accent {
    color: var(--accent);
  }
  .kpi b.warn {
    color: var(--warn);
  }
  section {
    margin-top: 26px;
  }
  h3 {
    margin: 0 0 12px;
    font-size: 12px;
    font-weight: 600;
    color: var(--muted);
  }
  .bars {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 8px;
  }
  .bars li {
    display: grid;
    grid-template-columns: 130px 1fr 64px;
    align-items: center;
    gap: 12px;
    font-size: 13px;
  }
  .name {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .track {
    height: 8px;
    border-radius: 8px;
    background: var(--surface-2);
    overflow: hidden;
  }
  .track i {
    display: block;
    height: 100%;
    border-radius: 8px;
    background: var(--text);
    opacity: 0.85;
    transform-origin: left;
    transition: transform 0.8s var(--ease-out);
  }
  .track i.warn {
    background: var(--warn);
    opacity: 1;
  }
  .val {
    text-align: right;
    color: var(--muted);
  }
  .cols {
    display: grid;
    grid-template-columns: 1.4fr 1fr;
    gap: 24px;
  }
  .sessions {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .sessions li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 0;
    border-bottom: 1px solid var(--line);
    font-size: 13px;
  }
  .time {
    color: var(--faint);
    width: 40px;
  }
  .what {
    flex: 1;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .muted {
    color: var(--muted);
  }
  .empty {
    margin: 0;
    color: var(--faint);
    font-size: 13px;
  }
</style>
