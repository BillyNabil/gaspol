<script>
  import { onMount } from 'svelte';
  import { fade, fly } from 'svelte/transition';
  import { quintOut } from 'svelte/easing';
  import { currentWindow, listen } from '../lib/tauri.js';
  import { api } from '../lib/api.js';
  import { ms } from '../lib/util.js';
  import Today from './Today.svelte';
  import Report from './Report.svelte';
  import Settings from './Settings.svelte';

  const TABS = [
    { id: 'today', name: 'Hari ini' },
    { id: 'report', name: 'Laporan' },
    { id: 'settings', name: 'Pengaturan' },
  ];

  const win = currentWindow();
  let tab = $state(TABS.some((t) => t.id === location.hash.slice(1)) ? location.hash.slice(1) : 'today');
  let st = $state(null);
  const idx = $derived(TABS.findIndex((t) => t.id === tab));

  onMount(() => {
    const unl = [];
    api.state().then((s) => (st = s));
    listen('tasks', (e) => st && (st.tasks = e.payload)).then((u) => unl.push(u));
    listen('session', (e) => st && (st.session = e.payload)).then((u) => unl.push(u));
    listen('nav', (e) => (tab = e.payload)).then((u) => unl.push(u));
    return () => unl.forEach((u) => u());
  });

  async function refresh() {
    st = await api.state();
  }
</script>

<div class="shell">
  <header class="bar" data-tauri-drag-region>
    <div class="brand" data-tauri-drag-region>
      <span class="logo" class:live={st?.session?.active}></span>
      <span data-tauri-drag-region>gaspol</span>
    </div>

    <nav class="seg tabs" style="--n:{TABS.length}">
      <span class="thumb" style="width: calc((100% - 6px) / {TABS.length}); transform: translateX({idx * 100}%)"></span>
      {#each TABS as t}
        <button class:on={tab === t.id} onclick={() => (tab = t.id)}>{t.name}</button>
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

  <main>
    {#if st}
      {#key tab}
        <div class="page" in:fly={{ y: 10, duration: ms(360), easing: quintOut }} out:fade={{ duration: ms(90) }}>
          {#if tab === 'today'}
            <Today {st} />
          {:else if tab === 'report'}
            <Report {st} />
          {:else}
            <Settings {st} onsaved={refresh} />
          {/if}
        </div>
      {/key}
    {/if}
  </main>
</div>

<style>
  .shell {
    height: 100%;
    display: grid;
    grid-template-rows: 52px 1fr;
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
  .tabs button {
    min-width: 96px;
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
</style>
