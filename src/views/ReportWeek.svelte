<script>
  import { fly } from 'svelte/transition';
  import { quintOut } from 'svelte/easing';
  import { api } from '../lib/api.js';
  import { fmtDur, todayStr, prettyDate, parseYmd, addDays, ms } from '../lib/util.js';

  let end = $state(todayStr());
  let w = $state(null);
  let ready = $state(false);

  $effect(() => {
    const e = end;
    ready = false;
    api.week(e).then((v) => {
      if (e === end) {
        w = v;
        requestAnimationFrame(() => (ready = true));
      }
    });
  });

  const HARI = ['Min', 'Sen', 'Sel', 'Rab', 'Kam', 'Jum', 'Sab'];
  const maxDay = $derived(w ? Math.max(3600, ...w.days.map((d) => d.focus + d.distracted)) : 1);
  const maxHeat = $derived(w ? Math.max(600, ...w.heat.flat()) : 1);
  const delta = $derived(w && w.prev_focus > 0 ? Math.round(((w.focus - w.prev_focus) / w.prev_focus) * 100) : null);
  const commits = $derived(w ? w.days.reduce((n, d) => n + d.commits, 0) : 0);
  const renders = $derived(w ? w.days.reduce((n, d) => n + d.renders, 0) : 0);

  function shift(n) {
    const next = addDays(end, n * 7);
    end = next > todayStr() ? todayStr() : next;
  }
</script>

{#if w}
  <div class="head">
    <button class="nav" aria-label="Minggu sebelumnya" onclick={() => shift(-1)}>‹</button>
    <h2 class="display">{prettyDate(w.days[0].date)} – {prettyDate(w.days[6].date)}</h2>
    <button class="nav" aria-label="Minggu berikutnya" disabled={end >= todayStr()} onclick={() => shift(1)}>›</button>
  </div>
  <p class="verdict">
    {#if w.best}Jam emasmu <b>{w.best}</b> — taruh tugas paling berat di situ.{:else}Belum cukup data untuk tahu jam emasmu.{/if}
    {#if delta !== null}<span class:up={delta >= 0} class:down={delta < 0}> Fokus {delta >= 0 ? '+' : ''}{delta}% dari minggu lalu.</span>{/if}
  </p>

  <div class="kpis">
    {#each [{ k: 'Fokus', v: fmtDur(w.focus), accent: true }, { k: 'Ngelantur', v: fmtDur(w.distracted), warn: true }, { k: 'Streak', v: `${w.stats.streak} hari` }, { k: 'Commit · render', v: `${commits} · ${renders}` }] as kpi, i}
      <div class="kpi" in:fly={{ y: 8, duration: ms(380), delay: ms(i * 50), easing: quintOut }}>
        <span>{kpi.k}</span>
        <b class="num display" class:accent={kpi.accent} class:warn={kpi.warn}>{kpi.v}</b>
      </div>
    {/each}
  </div>

  <section>
    <h3>Per hari</h3>
    <div class="cols7">
      {#each w.days as d, i}
        <div class="col">
          <div class="stack">
            <i class="dis" style="height: {ready ? (d.distracted / maxDay) * 100 : 0}%; transition-delay: {ms(i * 40)}ms"></i>
            <i class="foc" style="height: {ready ? (d.focus / maxDay) * 100 : 0}%; transition-delay: {ms(i * 40)}ms"></i>
          </div>
          <span class="num val">{fmtDur(d.focus)}</span>
          <span class="day" class:today={d.date === todayStr()}>{HARI[parseYmd(d.date).getDay()]}</span>
        </div>
      {/each}
    </div>
  </section>

  <section>
    <h3>Peta jam produktif</h3>
    <div class="heat">
      <span></span>
      {#each Array(24) as _, h}<span class="hh num">{h % 3 === 0 ? h : ''}</span>{/each}
      {#each w.heat as row, i}
        <span class="dl">{HARI[parseYmd(w.days[i].date).getDay()]}</span>
        {#each row as v, h}
          {@const dv = w.heat_dis[i]?.[h] ?? 0}
          <span
            class="cell"
            class:dis={dv > v && dv > 300}
            style="--a: {ready ? Math.min(1, Math.max(v, dv) / maxHeat) : 0}"
            title="{HARI[parseYmd(w.days[i].date).getDay()]} {h}:00 · {fmtDur(v)} produktif{dv ? ` · ${fmtDur(dv)} ngelantur` : ''}"
          ></span>
        {/each}
      {/each}
    </div>
  </section>
{/if}

<style>
  .head {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
  }
  h2 {
    margin: 0;
    font-size: 22px;
    font-weight: 650;
    letter-spacing: -0.02em;
    min-width: 260px;
    text-align: center;
  }
  .nav {
    width: 32px;
    height: 32px;
    border-radius: 50%;
    font-size: 20px;
    color: var(--muted);
  }
  .nav:hover:not(:disabled) {
    background: var(--surface-2);
    color: var(--text);
  }
  .nav:disabled {
    opacity: 0.25;
  }
  .verdict {
    margin: 6px 0 22px;
    text-align: center;
    color: var(--muted);
  }
  .verdict b {
    color: var(--text);
  }
  .up {
    color: var(--accent);
  }
  .down {
    color: var(--warn);
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
    font-size: 22px;
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
  .cols7 {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    gap: 10px;
  }
  .col {
    display: grid;
    justify-items: center;
    gap: 6px;
  }
  .stack {
    width: 100%;
    max-width: 46px;
    height: 120px;
    display: flex;
    flex-direction: column-reverse;
    border-radius: 8px;
    background: var(--surface);
    overflow: hidden;
  }
  .stack i {
    display: block;
    width: 100%;
    transition: height 0.8s var(--ease-out);
  }
  .foc {
    background: var(--accent);
  }
  .dis {
    background: var(--warn);
    opacity: 0.85;
  }
  .val {
    font-size: 11.5px;
    color: var(--muted);
  }
  .day {
    font-size: 12px;
    font-weight: 600;
    color: var(--faint);
  }
  .day.today {
    color: var(--text);
  }
  .heat {
    display: grid;
    grid-template-columns: 34px repeat(24, 1fr);
    gap: 3px;
    align-items: center;
  }
  .hh,
  .dl {
    font-size: 10.5px;
    color: var(--faint);
  }
  .cell {
    aspect-ratio: 1;
    border-radius: 3px;
    background: color-mix(in srgb, var(--accent) calc(var(--a) * 100%), var(--surface));
    transition: background-color 0.6s var(--ease);
  }
  .cell.dis {
    background: color-mix(in srgb, var(--warn) calc(var(--a) * 100%), var(--surface));
  }
</style>
