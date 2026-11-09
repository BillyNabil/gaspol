<script>
  import { flip } from 'svelte/animate';
  import { fly, fade, slide, scale } from 'svelte/transition';
  import { quintOut, backOut } from 'svelte/easing';
  import { api } from '../lib/api.js';
  import { onMount } from 'svelte';
  import { due, sortTasks, nextStep, fmtClock, fmtDur, ms, MODES, modeName, spent, timeOf } from '../lib/util.js';

  let { st, flash } = $props();

  let plan = $state(null);
  let presetBusy = $state(null);
  let updating = $state(null);

  onMount(() => {
    const load = () => api.plan().then((p) => (plan = p)).catch(() => {});
    load();
    const t = setInterval(load, 300000);
    return () => clearInterval(t);
  });

  async function runPreset(p) {
    presetBusy = p.id;
    try {
      await api.runPreset(p.id);
      flash?.(`Mode ${p.name} jalan`);
    } catch (e) {
      flash?.(String(e));
    } finally {
      presetBusy = null;
    }
  }

  async function closeDistractors() {
    const n = await api.closeDistractors();
    flash?.(n ? `${n} pengganggu ditutup` : 'Nggak ada pengganggu yang kebuka');
  }

  async function clientUpdate(project) {
    updating = project;
    try {
      const text = await api.clientUpdate(project);
      await navigator.clipboard.writeText(text);
      flash?.('Update untuk klien disalin ke clipboard');
    } catch (e) {
      flash?.(String(e));
    } finally {
      updating = null;
    }
  }

  const ratio = $derived(st.stats?.ratio ?? 1);
  function estLabel(t) {
    if (!t.est_minutes) return null;
    const real = Math.round(t.est_minutes * ratio);
    const left = Math.max(0, real * 60 - spent(t));
    return ratio > 1.15 || ratio < 0.85
      ? `estimasi ${fmtDur(t.est_minutes * 60)} → biasanya ${fmtDur(real * 60)} · sisa ±${fmtDur(left)}`
      : `estimasi ${fmtDur(t.est_minutes * 60)} · sisa ±${fmtDur(left)}`;
  }

  let input = $state('');
  let busy = $state(false);
  let note = $state('');
  let selectedId = $state(null);
  let openId = $state(null);
  let regenId = $state(null);
  let showDone = $state(false);
  let minutes = $state(st.settings.default_minutes);
  let mode = $state(st.settings.default_mode);

  const DURATIONS = [25, 50, 90];
  const open = $derived(sortTasks(st.tasks.filter((t) => !t.done)));
  const done = $derived(
    st.tasks
      .filter((t) => t.done)
      .sort((a, b) => (b.done_at ?? 0) - (a.done_at ?? 0))
      .slice(0, 30),
  );
  const focus = $derived(open.find((t) => t.id === selectedId) ?? open[0] ?? null);
  const s = $derived(st.session);
  const modeIdx = $derived(MODES.findIndex((m) => m.id === mode));
  const durIdx = $derived(DURATIONS.indexOf(minutes));

  async function add(e) {
    e.preventDefault();
    const text = input.trim();
    if (!text || busy) return;
    busy = true;
    note = '';
    try {
      const r = await api.capture(text);
      input = '';
      selectedId = r.task.id;
      if (r.warning) note = `AI gagal (${r.warning}), jadi pakai parser bawaan.`;
    } catch (err) {
      note = String(err);
    } finally {
      busy = false;
    }
  }

  async function regen(id) {
    regenId = id;
    note = '';
    try {
      await api.regen(id);
    } catch (err) {
      note = String(err);
    } finally {
      regenId = null;
    }
  }

  function toggleOpen(t) {
    selectedId = t.id;
    openId = openId === t.id ? null : t.id;
  }

  function saveField(t, patch) {
    api.updateTask({ ...$state.snapshot(t), ...patch });
  }
</script>

<div class="wrap">
  <form class="capture" onsubmit={add}>
    <span class="plus" class:busy>{#if busy}<i></i>{:else}+{/if}</span>
    <input
      bind:value={input}
      placeholder="Mau ngerjain apa? contoh: render teaser klien A sebelum jumat"
      spellcheck="false"
      disabled={busy}
    />
    <kbd>{st.settings.hotkey}</kbd>
  </form>
  {#if note}
    <p class="note" transition:slide={{ duration: ms(200) }}>{note}</p>
  {/if}

  {#if st.settings.presets?.length}
    <div class="presets">
      <span class="lbl">Mode kerja</span>
      {#each st.settings.presets as p (p.id)}
        <button class="chip-btn" disabled={presetBusy === p.id} onclick={() => runPreset(p)} title={p.open.join(' · ')}>
          {p.name}<span class="faint num">{p.minutes}m</span>
        </button>
      {/each}
      <span class="grow"></span>
      <button class="chip-btn ghost" onclick={closeDistractors}>Tutup pengganggu</button>
    </div>
  {/if}

  <!-- ===== Sekarang ===== -->
  <section class="hero" class:live={s.active} class:distracted={s.active && s.distracted}>
    {#key s.active}
      <div class="hero-in" in:fly={{ y: 12, duration: ms(420), easing: quintOut }}>
        {#if s.active}
          <div class="eyebrow">
            <span class="dot"></span>
            {s.distracted ? `Lagi di ${s.current}` : 'Lagi fokus'}
            <span class="sep">·</span>
            <span class="muted">{modeName(s.mode)}</span>
          </div>
          <div class="clock display num">{fmtClock(s.remaining)}{#if s.flow_ext}<span class="flow">+{Math.round(s.flow_ext / 60)}m flow</span>{/if}</div>
          {#if s.allow_left > 0}<p class="allow">Izin {s.allow_label || 'jeda'} · {fmtClock(s.allow_left)} lagi</p>{/if}
          <div class="progress"><i style="transform: scaleX({s.planned ? 1 - s.remaining / s.planned : 0})"></i></div>
          <h2>{s.task_title}</h2>
          {#if s.step}<p class="step"><span>Langkah</span>{s.step}</p>{/if}
          <div class="row between">
            <div class="stats num">
              <span><b>{fmtDur(s.focus_secs)}</b> fokus</span>
              <span><b>{s.distractions}×</b> ngelantur</span>
            </div>
            <button class="btn ghost" onclick={() => api.stop()}>Berhenti</button>
          </div>
        {:else if focus}
          {@const d = due(focus.deadline)}
          <div class="eyebrow">
            Sekarang
            {#if focus.project}<span class="sep">·</span><span class="muted">{focus.project}</span>{/if}
            {#if d}<span class="chip {d.tone}">{d.label}</span>{/if}
          </div>
          <h2 class="big display">{focus.title}</h2>
          {#if nextStep(focus)}
            <p class="step"><span>Langkah pertama</span>{nextStep(focus)}</p>
          {/if}
          <div class="controls">
            <div class="seg">
              <span
                class="thumb"
                style="width: calc((100% - 6px) / 3); transform: translateX({Math.max(durIdx, 0) * 100}%); opacity: {durIdx < 0 ? 0 : 1}"
              ></span>
              {#each DURATIONS as m}
                <button class:on={minutes === m} onclick={() => (minutes = m)}>{m}m</button>
              {/each}
            </div>
            <div class="seg">
              <span class="thumb" style="width: calc((100% - 6px) / 3); transform: translateX({modeIdx * 100}%)"></span>
              {#each MODES as m}
                <button class:on={mode === m.id} title={m.hint} onclick={() => (mode = m.id)}>{m.name}</button>
              {/each}
            </div>
            <button class="btn primary go" onclick={() => api.start(focus.id, minutes, mode)}>
              Mulai fokus
              <svg width="14" height="14" viewBox="0 0 14 14"
                ><path d="M3 7h8M7.5 3.5L11 7l-3.5 3.5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" /></svg
              >
            </button>
          </div>
        {:else}
          <div class="empty">
            <h2 class="big display">Kepala kosong, list kosong.</h2>
            <p>Tekan <kbd>{st.settings.hotkey}</kbd> di mana saja, ketik satu kalimat, beres.</p>
            <button class="btn" onclick={() => api.start(null, minutes, mode)}>Fokus bebas {minutes}m</button>
          </div>
        {/if}
      </div>
    {/key}
  </section>

  {#if plan && !s.active && (plan.events?.length || plan.free_text)}
    <section class="plan" in:fly={{ y: 8, duration: ms(320) }}>
      <div class="plan-head">
        <span>Hari ini</span>
        {#if plan.free_text}<span class="faint">kosong {plan.free_text}</span>{/if}
      </div>
      {#if plan.events?.length}
        <ul class="events">
          {#each plan.events.slice(0, 4) as e}
            <li><span class="num faint">{e.all_day ? 'seharian' : timeOf(e.start)}</span>{e.title}</li>
          {/each}
        </ul>
      {/if}
    </section>
  {/if}

  <!-- ===== Daftar ===== -->
  {#if open.length}
    <div class="list-head">
      <span>Antrian</span><span class="count num">{open.length}</span>
    </div>
  {/if}
  <ul class="list">
    {#each open as t (t.id)}
      {@const d = due(t.deadline)}
      <li
        class:sel={focus?.id === t.id && !s.active}
        class:expanded={openId === t.id}
        animate:flip={{ duration: ms(380), easing: quintOut }}
        in:fly={{ y: 10, duration: ms(320), easing: quintOut }}
        out:scale={{ start: 0.97, duration: ms(180) }}
      >
        <div class="row item" role="button" tabindex="0" onclick={() => toggleOpen(t)} onkeydown={(e) => e.key === 'Enter' && toggleOpen(t)}>
          <button
            class="check"
            aria-label="Tandai selesai"
            onclick={(e) => {
              e.stopPropagation();
              api.complete(t.id, true);
            }}
          ></button>
          <div class="meta">
            <div class="title">{t.title}</div>
            <div class="sub">
              {#if t.project}<span>{t.project}</span>{/if}
              {#if nextStep(t)}<span class="faint">→ {nextStep(t)}</span>{/if}
              {#if t.snoozes}<span class="snz">ditunda {t.snoozes}×</span>{/if}
            </div>
          </div>
          {#if spent(t) >= 60}<span class="spent num" title="{fmtDur(t.focus_secs)} sesi · {fmtDur(t.auto_secs ?? 0)} otomatis">{fmtDur(spent(t))}</span>{/if}
          {#if d}<span class="chip {d.tone}">{d.label}</span>{/if}
        </div>

        {#if openId === t.id}
          <div class="detail" transition:slide={{ duration: ms(280), easing: quintOut }}>
            <ol class="steps">
              {#each t.steps as step, i (i + step.text)}
                <li class:done={step.done} in:fly={{ x: -6, duration: ms(260), delay: ms(i * 35) }}>
                  <button class="tick" aria-label="Centang langkah" onclick={() => api.toggleStep(t.id, i)}>
                    {#if step.done}
                      <svg width="10" height="10" viewBox="0 0 10 10" in:scale={{ duration: ms(220), easing: backOut }}
                        ><path d="M2 5.2l2 2 4-4.4" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" /></svg
                      >
                    {/if}
                  </button>
                  <span>{step.text}</span>
                </li>
              {/each}
            </ol>
            {#if estLabel(t)}<p class="est">{estLabel(t)}</p>{/if}
            <div class="edit">
              <input
                class="field"
                value={t.title}
                onchange={(e) => saveField(t, { title: e.currentTarget.value.trim() || t.title })}
              />
              <input
                class="field date"
                type="date"
                value={t.deadline ?? ''}
                onchange={(e) => saveField(t, { deadline: e.currentTarget.value || null })}
              />
            </div>
            <div class="edit three">
              <input
                class="field"
                placeholder="Project / klien"
                value={t.project ?? ''}
                onchange={(e) => saveField(t, { project: e.currentTarget.value.trim() || null })}
              />
              <input
                class="field num"
                type="number"
                min="0"
                placeholder="Estimasi (menit)"
                value={t.est_minutes ?? ''}
                onchange={(e) => saveField(t, { est_minutes: Number(e.currentTarget.value) || null })}
              />
              <input
                class="field"
                placeholder="App sah: exe:blender, figma"
                title="Kerja di luar app ini saat sesi → ditegur pelan"
                value={(t.allowed ?? []).join(', ')}
                onchange={(e) => saveField(t, { allowed: e.currentTarget.value.split(',').map((x) => x.trim()).filter(Boolean) })}
              />
            </div>
            <div class="row actions">
              <button class="btn sm" disabled={regenId === t.id} onclick={() => regen(t.id)}>
                {regenId === t.id ? 'Mikir…' : 'Pecah lagi (AI)'}
              </button>
              <button class="btn sm primary" onclick={() => api.start(t.id, minutes, mode)}>Fokus {minutes}m</button>
              {#if t.project}
                <button class="btn sm" disabled={updating === t.project} onclick={() => clientUpdate(t.project)}>
                  {updating === t.project ? 'Nulis…' : 'Update klien'}
                </button>
              {/if}
              {#if t.template}<span class="chip">{t.template}</span>{/if}
              <span class="grow"></span>
              <button class="btn sm ghost danger" onclick={() => api.remove(t.id)}>Hapus</button>
            </div>
          </div>
        {/if}
      </li>
    {/each}
  </ul>

  {#if done.length}
    <button class="done-toggle" onclick={() => (showDone = !showDone)}>
      <span class="caret" class:on={showDone}>›</span> Selesai <span class="num">{done.length}</span>
    </button>
    {#if showDone}
      <ul class="list done-list" transition:slide={{ duration: ms(260), easing: quintOut }}>
        {#each done as t (t.id)}
          <li animate:flip={{ duration: ms(300) }} out:fade={{ duration: ms(120) }}>
            <div class="row item">
              <button class="check on" aria-label="Batalkan selesai" onclick={() => api.complete(t.id, false)}>
                <svg width="10" height="10" viewBox="0 0 10 10"
                  ><path d="M2 5.2l2 2 4-4.4" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" /></svg
                >
              </button>
              <div class="meta"><div class="title">{t.title}</div></div>
              {#if spent(t) >= 60}<span class="spent num">{fmtDur(spent(t))}</span>{/if}
            </div>
          </li>
        {/each}
      </ul>
    {/if}
  {/if}
</div>

<style>
  .presets {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 12px;
  }
  .presets .lbl {
    font-size: 12px;
    font-weight: 600;
    color: var(--faint);
    margin-right: 4px;
  }
  .chip-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 28px;
    padding: 0 11px;
    border-radius: 999px;
    font-size: 12.5px;
    font-weight: 600;
    background: var(--surface-2);
    border: 1px solid var(--line);
    color: var(--text);
    transition:
      background-color 0.18s var(--ease),
      transform 0.18s var(--spring);
  }
  .chip-btn:hover:not(:disabled) {
    background: var(--surface-3);
  }
  .chip-btn:active {
    transform: scale(0.96);
  }
  .chip-btn.ghost {
    background: transparent;
    color: var(--muted);
  }
  .flow {
    margin-left: 10px;
    font-size: 14px;
    font-weight: 600;
    color: var(--accent);
    vertical-align: middle;
  }
  .allow {
    margin: 4px 0 0;
    font-size: 12.5px;
    color: var(--warn);
  }
  .plan {
    margin: -10px 0 24px;
    padding: 12px 16px;
    border-radius: var(--r);
    background: var(--surface);
    border: 1px solid var(--line);
  }
  .plan-head {
    display: flex;
    justify-content: space-between;
    font-size: 12px;
    font-weight: 600;
    color: var(--muted);
  }
  .events {
    list-style: none;
    margin: 8px 0 0;
    padding: 0;
    display: grid;
    gap: 4px;
    font-size: 13px;
  }
  .events li {
    display: flex;
    gap: 10px;
  }
  .events .num {
    width: 54px;
  }
  .est {
    margin: 0 0 8px;
    font-size: 12px;
    color: var(--muted);
  }
  .edit.three {
    grid-template-columns: 1fr 140px 1.3fr;
    margin-top: 6px;
  }
  .wrap {
    max-width: 760px;
    margin: 0 auto;
    padding: 10px 28px 48px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .between {
    justify-content: space-between;
  }
  .grow {
    flex: 1;
  }
  .muted {
    color: var(--muted);
  }
  .faint {
    color: var(--faint);
  }
  kbd {
    font-family: inherit;
    font-size: 11px;
    font-weight: 600;
    color: var(--muted);
    padding: 2px 7px;
    border-radius: 6px;
    background: var(--surface-3);
    border: 1px solid var(--line);
  }

  /* capture */
  .capture {
    display: flex;
    align-items: center;
    gap: 12px;
    height: 50px;
    padding: 0 14px;
    border-radius: var(--r);
    background: var(--surface);
    border: 1px solid var(--line);
    transition:
      border-color 0.2s var(--ease),
      box-shadow 0.25s var(--ease);
  }
  .capture:focus-within {
    border-color: var(--line-2);
    box-shadow: 0 0 0 4px rgba(255, 255, 255, 0.025);
  }
  .capture input {
    flex: 1;
    min-width: 0;
    border: 0;
    outline: 0;
    background: transparent;
    font-size: 14.5px;
  }
  .capture input::placeholder {
    color: var(--faint);
  }
  .plus {
    width: 22px;
    height: 22px;
    display: grid;
    place-items: center;
    border-radius: 7px;
    background: var(--surface-3);
    color: var(--muted);
    font-weight: 600;
  }
  .plus.busy i {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    border: 2px solid var(--accent);
    border-right-color: transparent;
    animation: spin 0.7s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .note {
    margin: 8px 4px 0;
    font-size: 12.5px;
    color: var(--warn);
  }

  /* hero */
  .hero {
    position: relative;
    margin: 18px 0 28px;
    padding: 26px 28px 24px;
    border-radius: var(--r-lg);
    background: var(--surface);
    border: 1px solid var(--line);
    overflow: hidden;
    transition:
      border-color 0.5s var(--ease),
      box-shadow 0.5s var(--ease);
  }
  .hero.live {
    border-color: rgba(212, 255, 58, 0.22);
    box-shadow: 0 0 60px -20px rgba(212, 255, 58, 0.18);
  }
  .hero.distracted {
    border-color: rgba(255, 162, 76, 0.35);
    box-shadow: 0 0 60px -20px rgba(255, 162, 76, 0.22);
  }
  .eyebrow {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    font-weight: 600;
    color: var(--muted);
    letter-spacing: 0.01em;
  }
  .eyebrow .chip {
    margin-left: 4px;
  }
  .sep {
    color: var(--faint);
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--accent);
    animation: breathe 2s ease-in-out infinite;
  }
  .distracted .dot {
    background: var(--warn);
  }
  @keyframes breathe {
    50% {
      opacity: 0.35;
    }
  }
  h2 {
    margin: 10px 0 0;
    font-size: 17px;
    font-weight: 600;
    letter-spacing: -0.01em;
  }
  h2.big {
    font-size: 26px;
    line-height: 1.2;
    font-weight: 650;
    letter-spacing: -0.02em;
  }
  .clock {
    margin-top: 10px;
    font-size: 64px;
    line-height: 1;
    font-weight: 600;
    letter-spacing: -0.04em;
  }
  .progress {
    height: 3px;
    margin: 18px 0 4px;
    border-radius: 3px;
    background: var(--surface-3);
    overflow: hidden;
  }
  .progress i {
    display: block;
    height: 100%;
    background: var(--accent);
    transform-origin: left;
    transition: transform 1s linear;
  }
  .distracted .progress i {
    background: var(--warn);
  }
  .step {
    margin: 10px 0 0;
    display: flex;
    gap: 10px;
    align-items: baseline;
    font-size: 14.5px;
    color: var(--text);
  }
  .step span {
    flex: none;
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--accent);
  }
  .stats {
    display: flex;
    gap: 18px;
    margin-top: 18px;
    font-size: 13px;
    color: var(--muted);
  }
  .stats b {
    color: var(--text);
    font-weight: 600;
  }
  .controls {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
    margin-top: 22px;
  }
  .go {
    margin-left: auto;
    height: 40px;
    padding: 0 20px;
  }
  .go svg {
    transition: transform 0.25s var(--ease);
  }
  .go:hover svg {
    transform: translateX(3px);
  }
  .empty p {
    margin: 8px 0 18px;
    color: var(--muted);
  }

  /* list */
  .list-head {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0 4px 8px;
    font-size: 12px;
    font-weight: 600;
    color: var(--muted);
  }
  .count {
    color: var(--faint);
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .list > li {
    border-radius: var(--r);
    transition: background-color 0.2s var(--ease);
  }
  .list > li:hover,
  .list > li.expanded {
    background: var(--surface);
  }
  .list > li.sel .title {
    color: var(--accent);
  }
  .item {
    min-height: 52px;
    padding: 8px 12px;
    cursor: default;
    outline: none;
  }
  .meta {
    flex: 1;
    min-width: 0;
  }
  .title {
    font-weight: 550;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    transition: color 0.2s var(--ease);
  }
  .sub {
    display: flex;
    gap: 10px;
    font-size: 12px;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
  }
  .sub span {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .snz {
    color: var(--warn);
  }
  .spent {
    font-size: 12px;
    color: var(--faint);
  }
  .check {
    flex: none;
    width: 18px;
    height: 18px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    border: 1.5px solid var(--faint);
    color: var(--accent-ink);
    transition:
      border-color 0.2s var(--ease),
      background-color 0.2s var(--ease),
      transform 0.2s var(--ease);
  }
  .check:hover {
    border-color: var(--accent);
    transform: scale(1.08);
  }
  .check.on {
    background: var(--accent);
    border-color: var(--accent);
  }
  .detail {
    padding: 0 12px 14px 40px;
  }
  .steps {
    list-style: none;
    margin: 0 0 12px;
    padding: 0;
  }
  .steps li {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 5px 0;
    font-size: 13.5px;
    transition: color 0.25s var(--ease);
  }
  .steps li.done {
    color: var(--faint);
    text-decoration: line-through;
    text-decoration-color: var(--faint);
  }
  .tick {
    flex: none;
    margin-top: 2px;
    width: 15px;
    height: 15px;
    display: grid;
    place-items: center;
    border-radius: 4px;
    border: 1.5px solid var(--faint);
    color: var(--accent-ink);
    transition:
      background-color 0.2s var(--ease),
      border-color 0.2s var(--ease);
  }
  .done .tick {
    background: var(--accent);
    border-color: var(--accent);
  }
  .edit {
    display: grid;
    grid-template-columns: 1fr 160px;
    gap: 8px;
    margin-bottom: 10px;
  }
  .edit .field {
    height: 32px;
    font-size: 13px;
  }
  .actions {
    gap: 8px;
  }
  .danger:hover {
    color: var(--danger);
  }
  .done-toggle {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 18px 4px 6px;
    font-size: 12px;
    font-weight: 600;
    color: var(--faint);
    transition: color 0.2s var(--ease);
  }
  .done-toggle:hover {
    color: var(--muted);
  }
  .caret {
    display: inline-block;
    transition: transform 0.25s var(--ease);
  }
  .caret.on {
    transform: rotate(90deg);
  }
  .done-list .title {
    color: var(--faint);
  }
</style>
