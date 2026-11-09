<script>
  import { onMount } from 'svelte';
  import { fade } from 'svelte/transition';
  import { listen, demo } from '../lib/tauri.js';
  import { api } from '../lib/api.js';
  import { demoHud, demoLive } from '../lib/mock.js';
  import { fmtClock, fmtDur, ms } from '../lib/util.js';

  // Satu "pulau" yang berubah bentuk: pil timer ↔ teguran ↔ ringkasan.
  let msg = $state(demo ? (demoHud ?? null) : null);
  let sess = $state(demo === 'pill' ? demoLive : { active: false });
  let timer;

  const kind = $derived(msg?.kind ?? (sess.active ? 'pill' : null));
  const SIZES = {
    pill: [236, 40],
    nudge: [440, 132],
    strict: [360, 78],
    done: [400, 124],
    remind: [440, 136],
    report: [400, 124],
  };
  const sizeKey = $derived(kind === 'nudge' && msg?.strict ? 'strict' : kind);
  const size = $derived(SIZES[sizeKey] ?? SIZES.pill);

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
      else if (m.kind === 'done') autoClose(12000);
      else if (m.kind === 'report') autoClose(20000);
      else if (m.kind === 'remind') autoClose(30000);
      else autoClose(0);
    }).then((u) => unl.push(u));
    listen('session', (e) => (sess = e.payload)).then((u) => unl.push(u));
    return () => unl.forEach((u) => u());
  });

  function act(action, taskId = null) {
    clearTimeout(timer);
    msg = null;
    api.hud(action, taskId);
  }
</script>

<div class="stage">
  {#if kind}
    <div
      class="island {sizeKey}"
      class:warn={(kind === 'pill' && sess.distracted) || kind === 'nudge'}
      style="width:{size[0]}px; height:{size[1]}px"
      transition:fade={{ duration: ms(160) }}
    >
      {#key sizeKey + (msg?.task_id ?? '')}
        <div class="content" in:fade={{ duration: ms(220), delay: ms(140) }}>
          {#if kind === 'pill'}
            <div class="pill">
              <span class="dot"></span>
              <span class="t">{sess.task_title}</span>
              <span class="clock num">{fmtClock(sess.remaining)}</span>
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
          {:else if kind === 'done'}
            <div class="body">
              <p class="eyebrow acc">Sesi tuntas</p>
              <p class="main"><b class="num">{fmtDur(msg.focus)}</b> fokus · <span class="num">{msg.distractions}×</span> ngelantur</p>
              <p class="step">{msg.title}</p>
              <div class="btns">
                {#if msg.task_id}<button class="btn sm primary" onclick={() => act('again', msg.task_id)}>Lanjut 25m</button>{/if}
                <button class="btn sm ghost" onclick={() => act('dismiss')}>Istirahat dulu</button>
              </div>
            </div>
          {:else if kind === 'remind'}
            <div class="body">
              <p class="eyebrow warn-t">Deadline {msg.when}{msg.snoozes ? ` · udah ditunda ${msg.snoozes}×` : ''}</p>
              <p class="main"><b>{msg.title}</b></p>
              {#if msg.step}<p class="step">Mulai dari: {msg.step}</p>{/if}
              <div class="btns">
                <button class="btn sm primary" onclick={() => act('start', msg.task_id)}>Gas sekarang</button>
                <button class="btn sm ghost" onclick={() => act('snooze', msg.task_id)}>Nanti</button>
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
  .warn .dot {
    background: var(--warn);
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
