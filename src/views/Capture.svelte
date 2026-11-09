<script>
  import { onMount, tick } from 'svelte';
  import { fly, fade, scale } from 'svelte/transition';
  import { quintOut, backOut } from 'svelte/easing';
  import { currentWindow, listen, demo } from '../lib/tauri.js';
  import { api } from '../lib/api.js';
  import { due, ms } from '../lib/util.js';

  const win = currentWindow();
  let text = $state('');
  let phase = $state('idle'); // idle | loading | done | error
  let task = $state(null);
  let warn = $state('');
  let shown = $state(true);
  let el = $state();
  let timer;

  function reset() {
    text = '';
    phase = 'idle';
    task = null;
    warn = '';
  }

  async function hide() {
    clearTimeout(timer);
    shown = false;
    await new Promise((r) => setTimeout(r, ms(130)));
    await win.hide();
    reset();
  }

  onMount(() => {
    const unl = [];
    listen('capture-open', async () => {
      reset();
      shown = true;
      await tick();
      el?.focus();
    }).then((u) => unl.push(u));
    win
      .onFocusChanged(({ payload: focused }) => {
        if (!focused && phase !== 'loading') hide();
      })
      .then((u) => unl.push(u));
    el?.focus();
    if (demo) {
      text = 'render teaser klien Arunika sebelum jumat';
      if (demo === 'done') submit().then(() => clearTimeout(timer));
    }
    return () => unl.forEach((u) => u());
  });

  async function submit() {
    if (!text.trim() || phase === 'loading' || phase === 'done') return;
    phase = 'loading';
    try {
      const r = await api.capture(text);
      task = r.task;
      warn = r.warning ? `AI gagal: ${r.warning}` : '';
      phase = 'done';
      arm();
    } catch (e) {
      warn = String(e);
      phase = 'error';
    }
  }

  function arm() {
    clearTimeout(timer);
    timer = setTimeout(hide, 6000);
  }

  async function startNow() {
    clearTimeout(timer);
    await api.start(task.id);
    hide();
  }

  function onkey(e) {
    if (e.key === 'Escape') return hide();
    if (phase === 'done') {
      if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) return startNow();
      if (e.key === 'Enter') return hide();
    }
  }
</script>

<svelte:window onkeydown={onkey} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="stage" onclick={(e) => e.target === e.currentTarget && hide()}>
  <div class="card" class:shown class:loading={phase === 'loading'}>
    <form
      class="bar"
      onsubmit={(e) => {
        e.preventDefault();
        submit();
      }}
    >
      <svg class="glyph" width="18" height="18" viewBox="0 0 18 18"
        ><path d="M5 4l5 5-5 5M10 4l5 5-5 5" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" /></svg
      >
      <input
        bind:this={el}
        bind:value={text}
        placeholder="Apa yang harus beres?"
        spellcheck="false"
        readonly={phase === 'loading' || phase === 'done'}
        class:locked={phase === 'loading' || phase === 'done'}
      />
      {#if phase === 'idle' || phase === 'error'}
        <kbd in:fade={{ duration: ms(120) }}>Enter</kbd>
      {/if}
    </form>
    <div class="shimmer"></div>

    {#if phase === 'done' && task}
      {@const d = due(task.deadline)}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="result" in:fly={{ y: -6, duration: ms(380), easing: quintOut }} onmouseenter={() => clearTimeout(timer)} onmouseleave={arm}>
        <div class="saved">
          <span class="ok" in:scale={{ duration: ms(320), easing: backOut, start: 0.4 }}>
            <svg width="10" height="10" viewBox="0 0 10 10"
              ><path d="M2 5.2l2 2 4-4.4" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" /></svg
            >
          </span>
          Tersimpan
          {#if task.project}<span class="chip">{task.project}</span>{/if}
          {#if d}<span class="chip {d.tone}">{d.label}</span>{/if}
          {#if task.ai}<span class="chip accent">AI</span>{/if}
        </div>
        <h2>{task.title}</h2>
        <ol>
          {#each task.steps as s, i}
            <li class:first={i === 0} in:fly={{ x: -8, duration: ms(300), delay: ms(80 + i * 45), easing: quintOut }}>{s.text}</li>
          {/each}
        </ol>
        {#if warn}<p class="warn">{warn}</p>{/if}
        <div class="actions">
          <button class="btn primary" onclick={startNow}>Mulai fokus sekarang <kbd class="dark">Ctrl ↵</kbd></button>
          <button class="btn ghost" onclick={hide}>Nanti <kbd>↵</kbd></button>
        </div>
      </div>
    {:else if phase === 'error'}
      <p class="warn pad" in:fade>{warn}</p>
    {/if}
  </div>
</div>

<style>
  .stage {
    height: 100vh;
    padding: 16px 20px 40px;
  }
  .card {
    border-radius: 18px;
    background: rgba(19, 19, 21, 0.985);
    border: 1px solid rgba(255, 255, 255, 0.09);
    box-shadow:
      0 24px 60px -12px rgba(0, 0, 0, 0.7),
      0 0 0 1px rgba(0, 0, 0, 0.6);
    overflow: hidden;
    opacity: 0;
    transform: translateY(-8px) scale(0.985);
    transition:
      opacity 0.16s var(--ease),
      transform 0.32s var(--ease-out);
  }
  .card.shown {
    opacity: 1;
    transform: none;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 14px;
    height: 64px;
    padding: 0 20px;
  }
  .glyph {
    color: var(--accent);
    flex: none;
  }
  input {
    flex: 1;
    min-width: 0;
    border: 0;
    outline: 0;
    background: transparent;
    font-size: 19px;
    font-weight: 450;
    letter-spacing: -0.01em;
  }
  input::placeholder {
    color: var(--faint);
  }
  input.locked {
    color: var(--text);
    opacity: 0.6;
  }
  kbd {
    font-family: inherit;
    font-size: 11px;
    font-weight: 600;
    color: var(--muted);
    padding: 2px 7px;
    border-radius: 6px;
    background: var(--surface-3);
  }
  kbd.dark {
    background: rgba(0, 0, 0, 0.15);
    color: var(--accent-ink);
  }
  .shimmer {
    height: 1px;
    background: var(--line);
    position: relative;
    overflow: hidden;
  }
  .loading .shimmer::after {
    content: '';
    position: absolute;
    inset: 0;
    width: 40%;
    background: linear-gradient(90deg, transparent, var(--accent), transparent);
    animation: sweep 1.1s var(--ease) infinite;
  }
  @keyframes sweep {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(250%);
    }
  }
  .result {
    padding: 16px 22px 20px;
  }
  .saved {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    font-weight: 600;
    color: var(--muted);
  }
  .ok {
    width: 18px;
    height: 18px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    background: var(--accent);
    color: var(--accent-ink);
  }
  h2 {
    margin: 10px 0 8px;
    font-size: 20px;
    font-weight: 650;
    letter-spacing: -0.015em;
  }
  ol {
    margin: 0;
    padding: 0 0 0 18px;
    color: var(--muted);
    font-size: 13.5px;
  }
  ol li {
    padding: 2px 0;
  }
  ol li.first {
    color: var(--text);
  }
  ol li::marker {
    color: var(--faint);
    font-variant-numeric: tabular-nums;
  }
  .actions {
    display: flex;
    gap: 8px;
    margin-top: 16px;
  }
  .warn {
    margin: 10px 0 0;
    font-size: 12.5px;
    color: var(--warn);
  }
  .pad {
    padding: 0 22px 16px;
  }
</style>
