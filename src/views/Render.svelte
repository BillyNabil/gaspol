<script>
  import { onMount } from 'svelte';
  import { flip } from 'svelte/animate';
  import { fly, scale } from 'svelte/transition';
  import { quintOut } from 'svelte/easing';
  import { api } from '../lib/api.js';
  import { isTauri } from '../lib/tauri.js';
  import { fmtDur, ms, baseName } from '../lib/util.js';

  let { st, flash } = $props();

  const AFTER = [
    { id: 'none', name: 'Biarkan' },
    { id: 'sleep', name: 'Sleep' },
    { id: 'shutdown', name: 'Matikan PC' },
  ];

  let project = $state('');
  let comp = $state('');
  let single = $state(false);
  let frame = $state('1');
  let dragging = $state(false);
  let now = $state(Date.now() / 1000);

  const r = $derived(st.render);
  const projects = $derived([...new Set(st.tasks.filter((t) => !t.done && t.project).map((t) => t.project))]);
  const afterIdx = $derived(Math.max(0, AFTER.findIndex((a) => a.id === st.settings.after_queue)));
  const active = $derived(r.jobs.filter((j) => j.status === 'running' || j.status === 'queued'));
  const finished = $derived(r.jobs.filter((j) => j.status !== 'running' && j.status !== 'queued').reverse());

  onMount(() => {
    const t = setInterval(() => (now = Date.now() / 1000), 1000);
    let unlisten;
    if (isTauri) {
      import('@tauri-apps/api/webview').then(({ getCurrentWebview }) =>
        getCurrentWebview()
          .onDragDropEvent((e) => {
            const p = e.payload;
            if (p.type === 'over' || p.type === 'enter') dragging = true;
            else if (p.type === 'leave') dragging = false;
            else if (p.type === 'drop') {
              dragging = false;
              addFiles(p.paths);
            }
          })
          .then((u) => (unlisten = u)),
      );
    }
    return () => {
      clearInterval(t);
      unlisten?.();
    };
  });

  async function addFiles(paths) {
    const list = (Array.isArray(paths) ? paths : [paths]).filter(Boolean);
    let n = 0;
    for (const p of list) {
      try {
        const isAe = /\.aepx?$/i.test(p);
        await api.renderAdd(p, isAe ? comp || null : null, !isAe && single ? frame : null, project || null);
        n++;
      } catch (e) {
        flash(`${baseName(p)}: ${e}`);
      }
    }
    if (n) flash(`${n} file masuk antrian`);
  }

  async function pick() {
    const files = await api.pickFile([{ name: 'Blender / After Effects', extensions: ['blend', 'aep', 'aepx'] }]);
    if (files) addFiles(files);
  }

  async function setAfter(id) {
    st.settings.after_queue = id;
    await api.saveSettings($state.snapshot(st.settings));
  }

  async function toggleBreak() {
    st.settings.render_break = !st.settings.render_break;
    await api.saveSettings($state.snapshot(st.settings));
  }

  function eta(j) {
    if (j.status !== 'running' || !j.started || j.progress < 0.02) return null;
    const el = now - j.started;
    return Math.max(0, el / j.progress - el);
  }

  const STATUS = { queued: 'antri', running: 'render', done: 'beres', failed: 'gagal', canceled: 'batal' };
</script>

<div class="wrap" class:drag={dragging}>
  <div class="head">
    <div>
      <h1 class="display">Antrian render</h1>
      <p class="muted">Render jalan di background satu per satu. Kamu dikabari lewat island begitu selesai.</p>
    </div>
    <button class="btn primary" onclick={pick}>+ Tambah .blend / .aep</button>
  </div>

  <div class="opts">
    <label>
      <span>Project</span>
      <select class="field" bind:value={project}>
        <option value="">—</option>
        {#each projects as p}<option value={p}>{p}</option>{/each}
      </select>
    </label>
    <label>
      <span>Comp AE <em>kosong = isi Render Queue</em></span>
      <input class="field" bind:value={comp} placeholder="mis. MAIN 15s" spellcheck="false" />
    </label>
    <label>
      <span>Blender</span>
      <div class="inline">
        <button class="btn sm" class:on={!single} onclick={() => (single = false)}>Animasi</button>
        <button class="btn sm" class:on={single} onclick={() => (single = true)}>Frame</button>
        {#if single}<input class="field num tiny" bind:value={frame} />{/if}
      </div>
    </label>
  </div>

  {#if r.external}
    <div class="ext" in:fly={{ y: 6, duration: ms(260) }}>
      <span class="spin"></span>
      Render <b>{r.external}</b> terdeteksi · {fmtDur(r.external_secs)}
    </div>
  {/if}

  {#if !r.jobs.length}
    <div class="drop">
      <div class="big">⇣</div>
      <p>Seret file <b>.blend</b> atau <b>.aep</b> ke sini</p>
      <p class="faint small">
        Blender: {r.blender ? baseName(r.blender) + ' ✓' : 'tidak ketemu'} · aerender: {r.aerender ? 'ketemu ✓' : 'tidak ketemu'}
      </p>
    </div>
  {/if}

  <ul class="jobs">
    {#each [...active, ...finished] as j (j.id)}
      {@const e = eta(j)}
      <li class={j.status} animate:flip={{ duration: ms(320), easing: quintOut }} in:fly={{ y: 8, duration: ms(300) }} out:scale={{ start: 0.97, duration: ms(160) }}>
        <span class="kind {j.kind}">{j.kind === 'ae' ? 'Ae' : 'B'}</span>
        <div class="meta">
          <div class="name">{baseName(j.file)}{#if j.comp}<span class="faint"> · {j.comp}</span>{/if}</div>
          <div class="sub">
            <span class="chip {j.status === 'done' ? 'accent' : j.status === 'failed' ? 'danger' : ''}">{STATUS[j.status] ?? j.status}</span>
            {#if j.project}<span>{j.project}</span>{/if}
            {#if j.status === 'running' && j.total}<span class="num">frame {j.frame ?? 0}/{j.total}</span>{/if}
            {#if e !== null}<span class="num">± {fmtDur(e)} lagi</span>{/if}
            {#if j.status === 'done' && j.started && j.ended}<span class="num">{fmtDur(j.ended - j.started)}</span>{/if}
            {#if j.message && j.status !== 'done'}<span class="err">{j.message}</span>{/if}
          </div>
          {#if j.status === 'running'}
            <div class="prog"><i style="transform: scaleX({j.progress})"></i></div>
          {/if}
        </div>
        <div class="acts">
          {#if j.output && j.status === 'done'}<button class="btn sm" onclick={() => api.reveal(j.output)}>Buka hasil</button>{/if}
          {#if j.status === 'running' || j.status === 'queued'}
            <button class="btn sm ghost" onclick={() => api.renderCancel(j.id)}>Batal</button>
          {:else}
            {#if j.status !== 'done'}<button class="btn sm" onclick={() => api.renderRetry(j.id)}>Ulang</button>{/if}
            <button class="btn sm ghost" onclick={() => api.renderRemove(j.id)}>Hapus</button>
          {/if}
        </div>
      </li>
    {/each}
  </ul>
  {#if finished.length}
    <button class="link" onclick={() => api.renderClear()}>Bersihkan yang sudah selesai</button>
  {/if}

  <div class="foot">
    <label>
      <span>Setelah antrian habis</span>
      <div class="seg">
        <span class="thumb" style="width: calc((100% - 6px) / 3); transform: translateX({afterIdx * 100}%)"></span>
        {#each AFTER as a}
          <button class:on={st.settings.after_queue === a.id} onclick={() => setAfter(a.id)}>{a.name}</button>
        {/each}
      </div>
    </label>
    <label class="toggle">
      <span>Tawarkan istirahat saat render &gt; 5 menit</span>
      <button class="switch" class:on={st.settings.render_break} role="switch" aria-checked={st.settings.render_break} aria-label="Istirahat saat render" onclick={toggleBreak}><i></i></button>
    </label>
    <p class="faint small">
      {#if st.settings.organize_exports}
        Export baru dirapikan otomatis ke <code>{st.settings.export_root || 'folder asalnya'}\&lt;project&gt;\tanggal_nama_vNN</code>.
      {:else}
        Rapikan export otomatis masih mati — nyalakan di Pengaturan → Render.
      {/if}
    </p>
  </div>
</div>

<style>
  .wrap {
    max-width: 760px;
    margin: 0 auto;
    padding: 14px 28px 48px;
    min-height: 100%;
    transition: background-color 0.2s var(--ease);
  }
  .wrap.drag {
    background: rgba(212, 255, 58, 0.04);
    outline: 2px dashed rgba(212, 255, 58, 0.35);
    outline-offset: -10px;
    border-radius: 18px;
  }
  .head {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 16px;
  }
  h1 {
    margin: 0;
    font-size: 26px;
    font-weight: 650;
    letter-spacing: -0.02em;
  }
  .muted {
    color: var(--muted);
    margin: 4px 0 0;
    font-size: 13px;
  }
  .faint {
    color: var(--faint);
  }
  .small {
    font-size: 12px;
  }
  em {
    font-style: normal;
    font-weight: 500;
    color: var(--faint);
    margin-left: 4px;
  }
  .opts {
    display: grid;
    grid-template-columns: 1fr 1.3fr auto;
    gap: 12px;
    margin: 20px 0 14px;
  }
  label {
    display: grid;
    gap: 6px;
    align-content: start;
  }
  label > span {
    font-size: 12px;
    font-weight: 600;
    color: var(--muted);
  }
  .inline {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .tiny {
    width: 64px;
  }
  .btn.on {
    background: var(--accent-soft);
    color: var(--accent);
  }
  .ext {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 14px;
    margin-bottom: 12px;
    border-radius: var(--r);
    background: var(--surface);
    border: 1px solid var(--line);
    font-size: 13px;
    color: var(--muted);
  }
  .ext b {
    color: var(--text);
  }
  .spin {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    border: 2px solid var(--surface-3);
    border-top-color: var(--accent);
    animation: spin 0.9s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .drop {
    display: grid;
    place-items: center;
    gap: 2px;
    padding: 46px 0;
    border-radius: 18px;
    border: 1px dashed var(--line);
    color: var(--muted);
    text-align: center;
  }
  .drop p {
    margin: 0;
  }
  .drop .big {
    font-size: 28px;
    color: var(--faint);
  }
  .jobs {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 8px;
  }
  .jobs li {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 12px 14px;
    border-radius: var(--r);
    background: var(--surface);
    border: 1px solid var(--line);
  }
  .jobs li.running {
    border-color: rgba(212, 255, 58, 0.3);
  }
  .kind {
    flex: none;
    width: 34px;
    height: 34px;
    display: grid;
    place-items: center;
    border-radius: 10px;
    font-weight: 800;
    font-size: 13px;
  }
  .kind.ae {
    background: rgba(153, 153, 255, 0.14);
    color: #a9a9ff;
  }
  .kind.blender {
    background: rgba(255, 140, 60, 0.14);
    color: #ffa24c;
  }
  .meta {
    flex: 1;
    min-width: 0;
  }
  .name {
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .sub {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    align-items: center;
    margin-top: 4px;
    font-size: 12px;
    color: var(--muted);
  }
  .err {
    color: var(--danger);
    user-select: text;
  }
  .prog {
    height: 5px;
    margin-top: 9px;
    border-radius: 5px;
    background: var(--surface-3);
    overflow: hidden;
  }
  .prog i {
    display: block;
    height: 100%;
    background: var(--accent);
    transform-origin: left;
    transition: transform 0.6s var(--ease-out);
  }
  .acts {
    display: flex;
    gap: 6px;
  }
  .link {
    margin-top: 10px;
    font-size: 12.5px;
    color: var(--muted);
  }
  .link:hover {
    color: var(--text);
  }
  .foot {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 16px 20px;
    margin-top: 28px;
    padding-top: 20px;
    border-top: 1px solid var(--line);
  }
  .foot .seg {
    justify-self: start;
  }
  .foot p {
    grid-column: 1 / -1;
    margin: 0;
  }
  code {
    font-family: 'Cascadia Code', Consolas, monospace;
    font-size: 11.5px;
  }
  .switch {
    width: 42px;
    height: 24px;
    padding: 3px;
    border-radius: 999px;
    background: var(--surface-3);
    transition: background-color 0.25s var(--ease);
  }
  .switch i {
    display: block;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: var(--muted);
    transition:
      transform 0.35s var(--spring),
      background-color 0.25s var(--ease);
  }
  .switch.on {
    background: var(--accent);
  }
  .switch.on i {
    transform: translateX(18px);
    background: var(--accent-ink);
  }
</style>
