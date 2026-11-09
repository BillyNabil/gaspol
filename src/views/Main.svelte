<script>
  import { onMount } from 'svelte';
  import { fade, fly } from 'svelte/transition';
  import { quintOut } from 'svelte/easing';
  import { currentWindow, listen, isTauri } from '../lib/tauri.js';
  import { api } from '../lib/api.js';
  import { ms } from '../lib/util.js';
  import Today from './Today.svelte';
  import Render from './Render.svelte';
  import Report from './Report.svelte';
  import Settings from './Settings.svelte';

  const TABS = [
    { id: 'today', name: 'Hari ini' },
    { id: 'render', name: 'Render' },
    { id: 'report', name: 'Laporan' },
    { id: 'settings', name: 'Pengaturan' },
  ];

  const win = currentWindow();
  let tab = $state(TABS.some((t) => t.id === location.hash.slice(1)) ? location.hash.slice(1) : 'today');
  let st = $state(null);
  let toast = $state('');
  let update = $state(null);
  let updating = $state(false);
  const idx = $derived(TABS.findIndex((t) => t.id === tab));
  const lv = $derived(st?.stats);
  const lvPct = $derived(lv ? Math.min(1, (lv.xp - lv.level_from) / Math.max(1, lv.level_to - lv.level_from)) : 0);
  const running = $derived(st?.render?.jobs?.filter((j) => j.status === 'running' || j.status === 'queued').length ?? 0);

  let toastTimer;
  function flash(msg) {
    toast = msg;
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => (toast = ''), 3600);
  }

  onMount(() => {
    const unl = [];
    api.state().then((s) => (st = s));
    listen('tasks', (e) => st && (st.tasks = e.payload)).then((u) => unl.push(u));
    listen('session', (e) => st && (st.session = e.payload)).then((u) => unl.push(u));
    listen('render', (e) => st && (st.render = e.payload)).then((u) => unl.push(u));
    listen('nav', (e) => (tab = e.payload)).then((u) => unl.push(u));
    listen('toast', (e) => flash(e.payload)).then((u) => unl.push(u));
    listen('settings-changed', () => refresh()).then((u) => unl.push(u));
    const t = setInterval(() => api.stats().then((s) => st && (st.stats = s)), 60000);
    if (isTauri) {
      api
        .checkUpdate()
        .then((u) => (update = u))
        .catch(() => {});
    }
    return () => {
      clearInterval(t);
      unl.forEach((u) => u());
    };
  });

  async function refresh() {
    st = await api.state();
  }

  async function installUpdate() {
    updating = true;
    try {
      await update.downloadAndInstall();
      await api.relaunch();
    } catch (e) {
      flash(`Update gagal: ${e}`);
      updating = false;
    }
  }
</script>

<div class="shell">
  <header class="bar" data-tauri-drag-region>
    <div class="brand" data-tauri-drag-region>
      <span class="logo" class:live={st?.session?.active}></span>
      <span data-tauri-drag-region>gaspol</span>
      {#if lv}
        <span class="lv" title="{lv.xp} XP · {lv.level_to - lv.xp} XP lagi ke level {lv.level + 1}">
          <b>Lv {lv.level}</b>
          <span class="xpbar"><i style="transform: scaleX({lvPct})"></i></span>
          {#if lv.streak}<span class="streak">🔥 {lv.streak}</span>{/if}
        </span>
      {/if}
    </div>

    <nav class="seg tabs" style="--n:{TABS.length}">
      <span class="thumb" style="width: calc((100% - 6px) / {TABS.length}); transform: translateX({idx * 100}%)"></span>
      {#each TABS as t}
        <button class:on={tab === t.id} onclick={() => (tab = t.id)}>
          {t.name}{#if t.id === 'render' && running}<span class="badge num">{running}</span>{/if}
        </button>
      {/each}
    </nav>

    <div class="winbtns">
      <button aria-label="Minimize" onclick={() => win.minimize()}>
        <svg width="10" height="10" viewBox="0 0 10 10"><path d="M1 5h8" stroke="currentColor" stroke-width="1.2" /></svg>
      </button>
      <button aria-label="Tutup ke tray" class="close" onclick={() => win.close()}>
        <svg width="10" height="10" viewBox="0 0 10 10"
          ><path d="M1.5 1.5l7 7M8.5 1.5l-7 7" stroke="currentColor" stroke-width="1.2" /></svg
        >
      </button>
    </div>
  </header>

  <div>
    {#if update}
      <div class="update" transition:fly={{ y: -8, duration: ms(260) }}>
        Gaspol {update.version} sudah ada.
        <button class="btn sm primary" disabled={updating} onclick={installUpdate}>{updating ? 'Memasang…' : 'Pasang & restart'}</button>
        <button class="btn sm ghost" onclick={() => (update = null)}>Nanti</button>
      </div>
    {/if}
  </div>

  <main>
    {#if st}
      {#key tab}
        <div class="page" in:fly={{ y: 10, duration: ms(360), easing: quintOut }} out:fade={{ duration: ms(90) }}>
          {#if tab === 'today'}
            <Today {st} {flash} />
          {:else if tab === 'render'}
            <Render {st} {flash} />
          {:else if tab === 'report'}
            <Report {st} {flash} />
          {:else}
            <Settings {st} onsaved={refresh} />
          {/if}
        </div>
      {/key}
    {/if}
  </main>

  {#if toast}
    <div class="toast" transition:fly={{ y: 10, duration: ms(220) }}>{toast}</div>
  {/if}
</div>

<style>
  .shell {
    height: 100%;
    display: grid;
    grid-template-rows: 52px auto 1fr;
    background:
      radial-gradient(1200px 400px at 50% -220px, rgba(212, 255, 58, 0.045), transparent 70%),
      var(--bg);
  }
  .bar {
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    align-items: center;
    padding: 0 8px 0 18px;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    font-weight: 700;
    letter-spacing: -0.01em;
    color: var(--text);
  }
  .logo {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--faint);
    transition: background-color 0.3s var(--ease);
  }
  .logo.live {
    background: var(--accent);
    box-shadow: 0 0 0 0 rgba(212, 255, 58, 0.5);
    animation: ping 2.4s var(--ease) infinite;
  }
  @keyframes ping {
    70% {
      box-shadow: 0 0 0 8px rgba(212, 255, 58, 0);
    }
    100% {
      box-shadow: 0 0 0 0 rgba(212, 255, 58, 0);
    }
  }
  .lv {
    display: flex;
    align-items: center;
    gap: 7px;
    margin-left: 4px;
    font-size: 11.5px;
    font-weight: 600;
    color: var(--muted);
  }
  .lv b {
    color: var(--accent);
    font-weight: 700;
  }
  .xpbar {
    width: 46px;
    height: 4px;
    border-radius: 4px;
    background: var(--surface-3);
    overflow: hidden;
  }
  .xpbar i {
    display: block;
    height: 100%;
    background: var(--accent);
    transform-origin: left;
    transition: transform 0.8s var(--ease-out);
  }
  .streak {
    font-variant-numeric: tabular-nums;
  }
  .tabs button {
    min-width: 92px;
    position: relative;
  }
  .badge {
    display: inline-grid;
    place-items: center;
    min-width: 16px;
    height: 16px;
    margin-left: 6px;
    padding: 0 4px;
    border-radius: 8px;
    font-size: 10px;
    font-weight: 700;
    background: var(--accent);
    color: var(--accent-ink);
  }
  .winbtns {
    justify-self: end;
    display: flex;
    gap: 2px;
  }
  .winbtns button {
    width: 38px;
    height: 30px;
    display: grid;
    place-items: center;
    border-radius: 8px;
    color: var(--muted);
    transition:
      background-color 0.15s var(--ease),
      color 0.15s var(--ease);
  }
  .winbtns button:hover {
    background: var(--surface-2);
    color: var(--text);
  }
  .winbtns .close:hover {
    background: #c42b1c;
    color: white;
  }
  .update {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 10px;
    padding: 6px 12px 8px;
    font-size: 13px;
    color: var(--accent);
  }
  main {
    position: relative;
    overflow: hidden;
  }
  .page {
    position: absolute;
    inset: 0;
    overflow-y: auto;
    overflow-x: hidden;
  }
  .toast {
    position: fixed;
    left: 50%;
    bottom: 20px;
    translate: -50% 0;
    max-width: 80%;
    padding: 8px 14px;
    border-radius: 999px;
    font-size: 12.5px;
    background: var(--surface-2);
    border: 1px solid var(--line);
    color: var(--text);
    user-select: text;
    z-index: 20;
  }
</style>
