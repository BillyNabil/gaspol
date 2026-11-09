<script>
  import { onMount } from 'svelte';
  import { fade, fly } from 'svelte/transition';
  import { quintOut } from 'svelte/easing';
  import { listen, demo } from '../lib/tauri.js';
  import { api } from '../lib/api.js';
  import { demoHud, demoLive, demoRender } from '../lib/mock.js';
  import { fmtClock, fmtDur, ms, baseName } from '../lib/util.js';

  // Satu "pulau" yang berubah bentuk: pil timer ↔ teguran ↔ ringkasan.
  let msg = $state(demo && demo !== 'pill' && demo !== 'renderpill' ? (demoHud ?? null) : null);
  let sess = $state(demo === 'pill' ? demoLive : { active: false });
  let rjobs = $state(demo === 'renderpill' ? demoRender.jobs : []);
  let timer;

  const running = $derived(rjobs.find((j) => j.status === 'running'));
  const kind = $derived(msg?.kind ?? (sess.active || running ? 'pill' : null));
  const SIZES = {
    pill: [236, 40],
    nudge: [440, 132],
    strict: [360, 78],
    done: [400, 124],
    remind: [440, 136],
    report: [400, 124],
    intent: [440, 120],
    offtask: [420, 120],
    break: [400, 116],
    render: [420, 124],
    render_done: [420, 120],
    power: [400, 104],
    plan: [460, 176],
  };
  const sizeKey = $derived(kind === 'nudge' && msg?.strict ? 'strict' : kind);
  const size = $derived(SIZES[sizeKey] ?? SIZES.pill);

  const AUTO = { done: 12000, report: 20000, remind: 30000, intent: 25000, offtask: 15000, break: 30000, render: 30000, render_done: 20000, plan: 60000 };

  function autoClose(after) {
    clearTimeout(timer);
    if (after) timer = setTimeout(() => act('dismiss'), after);
  }

  onMount(() => {
    const unl = [];
    listen('hud', (e) => {
      const m = e.payload;
      msg = m.kind === 'pill' ? null : m;
      if (m.kind === 'nudge' && m.strict) autoClose(3200);
      else autoClose(AUTO[m.kind] ?? 0);
    }).then((u) => unl.push(u));
    listen('session', (e) => (sess = e.payload)).then((u) => unl.push(u));
    listen('render', (e) => (rjobs = e.payload.jobs)).then((u) => unl.push(u));
    return () => unl.forEach((u) => u());
  });

  function act(action, taskId = null, value = null) {
    clearTimeout(timer);
    msg = null;
    api.hud(action, taskId, value);
  }
</script>

<div class="stage">
  {#if kind}
    <div
      class="island {sizeKey}"
      class:warn={(kind === 'pill' && sess.distracted && !sess.allow_left) || kind === 'nudge' || kind === 'intent'}
      style="width:{size[0]}px; height:{size[1]}px"
      transition:fade={{ duration: ms(160) }}
    >
      {#key sizeKey + (msg?.task_id ?? '') + (msg?.name ?? '')}
        <div class="content" in:fade={{ duration: ms(220), delay: ms(140) }}>
          {#if kind === 'pill' && sess.active}
            <div class="pill">
              <span class="dot" class:allow={sess.allow_left > 0}></span>
              <span class="t">{sess.allow_left > 0 ? `Izin ${sess.allow_label}` : sess.task_title}</span>
              {#if running}<span class="r num" title="Render {baseName(running.file)}">R {Math.round(running.progress * 100)}%</span>{/if}
              <span class="clock num">{fmtClock(sess.allow_left > 0 ? sess.allow_left : sess.remaining)}</span>
            </div>
          {:else if kind === 'pill' && running}
            <div class="pill">
              <span class="ring" style="--p: {running.progress}"></span>
              <span class="t">Render {baseName(running.file)}</span>
              <span class="clock num">{Math.round(running.progress * 100)}%</span>
            </div>
          {:else if kind === 'nudge' && msg.strict}
            <div class="strict">
              <b>{msg.label} ditutup.</b>
              <span>Balik ke: {msg.title}</span>
            </div>
          {:else if kind === 'nudge'}
            <div class="body">
              <p class="eyebrow warn-t">Hei, ini {msg.label}.</p>
              <p class="main">Balik ke <b>{msg.title}</b></p>
              {#if msg.step}<p class="step">→ {msg.step}</p>{/if}
              <div class="btns">
                <button class="btn sm primary" onclick={() => act('back')}>Balik kerja</button>
                <button class="btn sm ghost" onclick={() => act('dismiss')}>Sebentar</button>
                <button class="btn sm ghost" onclick={() => act('stop')}>Udahan</button>
              </div>
            </div>
          {:else if kind === 'intent'}
            <div class="body">
              <p class="eyebrow warn-t">Buka {msg.label} — mau berapa lama?</p>
              <p class="main">Habis itu balik ke <b>{msg.title}</b></p>
              <div class="btns">
                {#each [2, 5, 10] as m}
                  <button class="btn sm" onclick={() => act('allow', null, m)}>{m} menit</button>
                {/each}
                <button class="btn sm primary" onclick={() => act('back')}>Nggak jadi</button>
              </div>
            </div>
          {:else if kind === 'offtask'}
            <div class="body">
              <p class="eyebrow warn-t">Lagi di {msg.label}</p>
              <p class="main">Ini bukan app untuk <b>{msg.title}</b></p>
              <div class="btns">
                <button class="btn sm primary" onclick={() => act('back')}>Balik kerja</button>
                <button class="btn sm ghost" onclick={() => act('dismiss')}>Memang perlu</button>
              </div>
            </div>
          {:else if kind === 'done'}
            <div class="body">
              <p class="eyebrow acc">Sesi tuntas{msg.flow ? ` · +${Math.round(msg.flow / 60)}m flow` : ''}</p>
              <p class="main"><b class="num">{fmtDur(msg.focus)}</b> fokus · <span class="num">{msg.distractions}×</span> ngelantur</p>
              <p class="step">{msg.title}</p>
              <div class="btns">
                {#if msg.task_id}<button class="btn sm primary" onclick={() => act('again', msg.task_id)}>Lanjut 25m</button>{/if}
                <button class="btn sm ghost" onclick={() => act('dismiss')}>Istirahat dulu</button>
              </div>
            </div>
          {:else if kind === 'remind'}
            <div class="body">
              <p class="eyebrow warn-t">
                Deadline {msg.when}{msg.need_min ? ` · butuh ±${fmtDur(msg.need_min * 60)}` : ''}{msg.snoozes ? ` · udah ditunda ${msg.snoozes}×` : ''}
              </p>
              <p class="main"><b>{msg.title}</b></p>
              {#if msg.step}<p class="step">Mulai dari: {msg.step}</p>{/if}
              <div class="btns">
                <button class="btn sm primary" onclick={() => act('start', msg.task_id)}>Gas sekarang</button>
                <button class="btn sm ghost" onclick={() => act('snooze', msg.task_id)}>Nanti</button>
              </div>
            </div>
          {:else if kind === 'break'}
            <div class="body">
              <p class="eyebrow acc">Udah {fmtDur(msg.mins * 60)} nonstop</p>
              <p class="main">Minum, berdiri, lihat jauh 20 detik.</p>
              <div class="btns">
                <button class="btn sm primary" onclick={() => act('dismiss')}>Oke, rehat bentar</button>
                <button class="btn sm ghost" onclick={() => act('dismiss')}>Nanti</button>
              </div>
            </div>
          {:else if kind === 'render'}
            <div class="body">
              <p class="eyebrow acc">Render {msg.label} jalan · {fmtDur(msg.secs)}</p>
              <p class="main">Istirahat dulu? Nanti dipanggil pas selesai.</p>
              <div class="btns">
                <button class="btn sm primary" onclick={() => act('render_break')}>Istirahat</button>
                <button class="btn sm ghost" onclick={() => act('dismiss')}>Lanjut kerja</button>
              </div>
            </div>
          {:else if kind === 'render_done'}
            <div class="body">
              <p class="eyebrow" class:acc={msg.ok} class:warn-t={!msg.ok}>{msg.ok ? 'Render selesai' : 'Render gagal'} · {fmtDur(msg.secs)}</p>
              <p class="main"><b>{msg.name}</b></p>
              <p class="step">{msg.ok ? (msg.output ? baseName(msg.output) : 'Balik ke kerjaan!') : msg.message}</p>
              <div class="btns">
                {#if msg.output}
                  <button class="btn sm primary" onclick={() => (api.reveal(msg.output), act('dismiss'))}>Buka hasil</button>
                {:else if !msg.external}
                  <button class="btn sm primary" onclick={() => act('open_render')}>Lihat antrian</button>
                {/if}
                <button class="btn sm ghost" onclick={() => act('dismiss')}>Tutup</button>
              </div>
            </div>
          {:else if kind === 'power'}
            <div class="body">
              <p class="eyebrow acc">Antrian render habis</p>
              <p class="main">{msg.action === 'shutdown' ? 'PC mati 2 menit lagi.' : 'PC sleep 1 menit lagi.'}</p>
              <div class="btns">
                <button class="btn sm primary" onclick={() => act('cancel_power')}>Batalkan</button>
                <button class="btn sm ghost" onclick={() => act('dismiss')}>Oke</button>
              </div>
            </div>
          {:else if kind === 'plan'}
            <div class="body">
              <p class="eyebrow acc">Rencana hari ini{msg.free ? ` · kosong ${msg.free}` : ''}</p>
              <ol class="plan">
                {#each msg.tasks as t, i}
                  <li in:fly={{ x: -6, duration: ms(260), delay: ms(200 + i * 60), easing: quintOut }}>
                    <b>{t.title}</b>
                    {#if t.step}<span>→ {t.step}</span>{/if}
                  </li>
                {/each}
              </ol>
              <div class="btns">
                <button class="btn sm primary" onclick={() => act('start', msg.tasks[0]?.id)}>Mulai yang pertama</button>
                <button class="btn sm ghost" onclick={() => act('open')}>Lihat semua</button>
                <button class="btn sm ghost" onclick={() => act('dismiss')}>Nanti</button>
              </div>
            </div>
          {:else if kind === 'report'}
            <div class="body">
              <p class="eyebrow acc">Laporan hari ini</p>
              <p class="main">
                <b class="num">{fmtDur(msg.focus)}</b> fokus · <span class="num warn-t">{fmtDur(msg.distracted)}</span> ngelantur
              </p>
              {#if msg.top}<p class="step">Paling lama di {msg.top}</p>{/if}
              <div class="btns">
                <button class="btn sm primary" onclick={() => act('report')}>Lihat detail</button>
                <button class="btn sm ghost" onclick={() => act('dismiss')}>Tutup</button>
              </div>
            </div>
          {/if}
        </div>
      {/key}
    </div>
  {/if}
</div>

<style>
  .stage {
    height: 100vh;
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding-top: 4px;
  }
  .island {
    position: relative;
    overflow: hidden;
    border-radius: 22px;
    background: #0d0d0e;
    border: 1px solid rgba(255, 255, 255, 0.09);
    box-shadow: 0 10px 30px -8px rgba(0, 0, 0, 0.6);
    transition:
      width 0.5s var(--spring),
      height 0.5s var(--spring),
      border-color 0.3s var(--ease),
      border-radius 0.5s var(--spring);
  }
  .island.pill {
    border-radius: 20px;
  }
  .island.warn {
    border-color: rgba(255, 162, 76, 0.45);
  }
  .content {
    position: absolute;
    inset: 0;
  }
  .pill {
    height: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 16px;
    font-size: 13px;
  }
  .dot {
    flex: none;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--accent);
    animation: breathe 2s ease-in-out infinite;
  }
  .warn .dot,
  .dot.allow {
    background: var(--warn);
  }
  .ring {
    flex: none;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: conic-gradient(var(--accent) calc(var(--p) * 360deg), var(--surface-3) 0);
    mask: radial-gradient(circle, transparent 3.5px, #000 4px);
    transition: background 0.6s var(--ease);
  }
  @keyframes breathe {
    50% {
      opacity: 0.35;
    }
  }
  .t {
    flex: 1;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--muted);
  }
  .r {
    font-size: 11px;
    font-weight: 700;
    color: var(--accent);
  }
  .clock {
    font-weight: 600;
  }
  .body {
    padding: 14px 18px;
  }
  p {
    margin: 0;
  }
  .eyebrow {
    font-size: 11.5px;
    font-weight: 700;
    letter-spacing: 0.02em;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .warn-t {
    color: var(--warn);
  }
  .acc {
    color: var(--accent);
  }
  .main {
    margin-top: 3px;
    font-size: 14.5px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .main b {
    font-weight: 650;
  }
  .step {
    margin-top: 2px;
    font-size: 12.5px;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .btns {
    display: flex;
    gap: 6px;
    margin-top: 10px;
  }
  .plan {
    margin: 6px 0 0;
    padding-left: 18px;
    display: grid;
    gap: 4px;
    font-size: 13px;
  }
  .plan li {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .plan li::marker {
    color: var(--faint);
  }
  .plan span {
    margin-left: 6px;
    color: var(--muted);
    font-size: 12px;
  }
  .strict {
    height: 100%;
    display: flex;
    flex-direction: column;
    justify-content: center;
    padding: 0 20px;
    font-size: 13px;
  }
  .strict b {
    color: var(--warn);
    font-size: 14.5px;
  }
  .strict span {
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
