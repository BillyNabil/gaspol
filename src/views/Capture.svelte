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
  let rec = $state('off'); // off | on | busy
  let image = $state(null); // { b64, mime, url }
  let timer;
  let recorder;
  let chunks = [];

  function reset() {
    text = '';
    phase = 'idle';
    task = null;
    warn = '';
    image = null;
  }

  async function hide() {
    clearTimeout(timer);
    stopRec(true);
    shown = false;
    await new Promise((r) => setTimeout(r, ms(130)));
    await win.hide();
    reset();
  }

  onMount(() => {
    const unl = [];
    listen('capture-open', async (e) => {
      reset();
      if (typeof e.payload === 'string' && e.payload.trim()) text = e.payload.trim();
      shown = true;
      await tick();
      el?.focus();
      el?.setSelectionRange(text.length, text.length);
    }).then((u) => unl.push(u));
    win
      .onFocusChanged(({ payload: focused }) => {
        if (!focused && phase !== 'loading' && rec === 'off') hide();
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
    if ((!text.trim() && !image) || phase === 'loading' || phase === 'done') return;
    phase = 'loading';
    try {
      const r = image ? await api.captureImage(image.b64, image.mime, text) : await api.capture(text);
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

  function toB64(blob) {
    return new Promise((res, rej) => {
      const r = new FileReader();
      r.onload = () => res(String(r.result));
      r.onerror = rej;
      r.readAsDataURL(blob);
    });
  }

  // Screenshot chat klien → tugas + checklist (#12).
  async function onpaste(e) {
    const item = [...(e.clipboardData?.items ?? [])].find((i) => i.type.startsWith('image/'));
    if (!item) return;
    e.preventDefault();
    const file = item.getAsFile();
    if (!file) return;
    const url = await toB64(file);
    image = { b64: url.split(',')[1], mime: file.type, url };
    await tick();
    el?.focus();
  }

  // Suara → teks (#13).
  async function toggleRec() {
    if (rec === 'on') return stopRec(false);
    if (rec !== 'off') return;
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      chunks = [];
      recorder = new MediaRecorder(stream, { mimeType: 'audio/webm;codecs=opus' });
      recorder.ondataavailable = (e) => e.data.size && chunks.push(e.data);
      recorder.onstop = async () => {
        stream.getTracks().forEach((t) => t.stop());
        if (recorder._discard) return;
        rec = 'busy';
        try {
          const blob = new Blob(chunks, { type: 'audio/webm' });
          const b64 = (await toB64(blob)).split(',')[1];
          const said = await api.transcribe(b64, 'audio/webm');
          text = text ? `${text} ${said}` : said;
          rec = 'off';
          await tick();
          submit();
        } catch (err) {
          rec = 'off';
          warn = String(err);
          phase = 'error';
        }
      };
      recorder.start();
      rec = 'on';
    } catch (err) {
      warn = `Mikrofon tidak bisa dipakai: ${err?.message ?? err}`;
      phase = 'error';
    }
  }

  function stopRec(discard) {
    if (recorder && recorder.state === 'recording') {
      recorder._discard = discard;
      recorder.stop();
    }
    if (discard) rec = 'off';
  }

  function onkey(e) {
    if (e.key === 'Escape') return hide();
    if (phase === 'done') {
      if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) return startNow();
      if (e.key === 'Enter') return hide();
    }
  }

  function inputKey(e) {
    if (e.key === 'Enter' && !e.shiftKey && !e.ctrlKey && phase !== 'done') {
      e.preventDefault();
      submit();
    }
  }
</script>

<svelte:window onkeydown={onkey} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="stage" onclick={(e) => e.target === e.currentTarget && hide()}>
  <div class="card" class:shown class:loading={phase === 'loading' || rec === 'busy'}>
    <div class="bar">
      <svg class="glyph" width="18" height="18" viewBox="0 0 18 18"
        ><path d="M5 4l5 5-5 5M10 4l5 5-5 5" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" /></svg
      >
      {#if image}
        <span class="thumbimg" in:scale={{ duration: ms(220), easing: backOut }}>
          <img src={image.url} alt="" />
          <button aria-label="Buang gambar" onclick={() => (image = null)}>×</button>
        </span>
      {/if}
      <textarea
        bind:this={el}
        bind:value={text}
        rows="1"
        placeholder={image ? 'Catatan (opsional) — mis. "revisi klien Arunika, deadline jumat"' : rec === 'on' ? 'Ngomong aja… klik mic lagi kalau selesai' : 'Apa yang harus beres?'}
        spellcheck="false"
        readonly={phase === 'loading' || phase === 'done'}
        class:locked={phase === 'loading' || phase === 'done'}
        onkeydown={inputKey}
        {onpaste}
      ></textarea>
      <button class="mic" class:on={rec === 'on'} class:busy={rec === 'busy'} aria-label="Rekam suara" title="Rekam suara" onclick={toggleRec} disabled={phase === 'loading' || phase === 'done'}>
        <svg width="16" height="16" viewBox="0 0 16 16"
          ><rect x="5.5" y="1.5" width="5" height="8.5" rx="2.5" fill="none" stroke="currentColor" stroke-width="1.5" /><path
            d="M3 7.5a5 5 0 0 0 10 0M8 12.5V15"
            fill="none"
            stroke="currentColor"
            stroke-width="1.5"
            stroke-linecap="round"
          /></svg
        >
      </button>
      {#if (phase === 'idle' || phase === 'error') && rec === 'off'}
        <kbd in:fade={{ duration: ms(120) }}>Enter</kbd>
      {/if}
    </div>
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
          {#if task.template}<span class="chip">{task.template}</span>{/if}
          {#if task.ai}<span class="chip accent">AI</span>{/if}
        </div>
        <h2>{task.title}</h2>
        <ol>
          {#each task.steps.slice(0, 7) as s, i}
            <li class:first={i === 0} in:fly={{ x: -8, duration: ms(300), delay: ms(80 + i * 45), easing: quintOut }}>{s.text}</li>
          {/each}
          {#if task.steps.length > 7}<li class="more">+{task.steps.length - 7} langkah lagi</li>{/if}
        </ol>
        {#if warn}<p class="warn">{warn}</p>{/if}
        <div class="actions">
          <button class="btn primary" onclick={startNow}>Mulai fokus sekarang <kbd class="dark">Ctrl ↵</kbd></button>
          <button class="btn ghost" onclick={hide}>Nanti <kbd>↵</kbd></button>
        </div>
      </div>
    {:else if phase === 'error'}
      <p class="warn pad" in:fade>{warn}</p>
    {:else if phase === 'idle' && !text && !image && rec === 'off'}
      <p class="tips" in:fade={{ duration: ms(200), delay: ms(150) }}>Tempel screenshot chat klien (Ctrl+V) · daftar per baris jadi checklist · klik mic untuk ngomong</p>
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
    min-height: 64px;
    padding: 10px 16px 10px 20px;
  }
  .glyph {
    color: var(--accent);
    flex: none;
  }
  textarea {
    flex: 1;
    min-width: 0;
    border: 0;
    outline: 0;
    resize: none;
    background: transparent;
    font: inherit;
    font-size: 19px;
    font-weight: 450;
    letter-spacing: -0.01em;
    line-height: 1.35;
    color: var(--text);
    field-sizing: content;
    max-height: 150px;
    padding: 6px 0;
  }
  textarea::placeholder {
    color: var(--faint);
  }
  textarea.locked {
    opacity: 0.6;
  }
  .thumbimg {
    position: relative;
    flex: none;
    width: 44px;
    height: 44px;
    border-radius: 10px;
    overflow: hidden;
    border: 1px solid var(--line);
  }
  .thumbimg img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .thumbimg button {
    position: absolute;
    top: 1px;
    right: 1px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    font-size: 12px;
    line-height: 1;
    background: rgba(0, 0, 0, 0.7);
    color: white;
  }
  .mic {
    flex: none;
    width: 34px;
    height: 34px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    color: var(--muted);
    transition:
      background-color 0.2s var(--ease),
      color 0.2s var(--ease);
  }
  .mic:hover:not(:disabled) {
    background: var(--surface-3);
    color: var(--text);
  }
  .mic.on {
    background: var(--danger);
    color: white;
    animation: pulse 1.4s ease-in-out infinite;
  }
  .mic.busy {
    color: var(--accent);
  }
  @keyframes pulse {
    50% {
      box-shadow: 0 0 0 6px rgba(255, 90, 90, 0.18);
    }
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
  ol li.more {
    list-style: none;
    color: var(--faint);
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
  .tips {
    margin: 0;
    padding: 10px 22px 12px;
    font-size: 12px;
    color: var(--faint);
  }
</style>
