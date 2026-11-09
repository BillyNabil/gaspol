<script>
  import { slide } from 'svelte/transition';
  import { api } from '../lib/api.js';
  import { fmtDur, fmtMoney, todayStr, ymd, ms } from '../lib/util.js';

  let { st, flash } = $props();

  const RANGES = [
    { id: 'month', name: 'Bulan ini' },
    { id: 'last', name: 'Bulan lalu' },
    { id: '30', name: '30 hari' },
  ];
  let range = $state('month');
  let rows = $state([]);
  let open = $state(null);
  let busy = $state(null);
  const idx = $derived(RANGES.findIndex((r) => r.id === range));
  const cur = $derived(st.settings.currency || 'Rp');

  const period = $derived.by(() => {
    const now = new Date();
    if (range === 'month') return [ymd(new Date(now.getFullYear(), now.getMonth(), 1)), todayStr()];
    if (range === 'last') return [ymd(new Date(now.getFullYear(), now.getMonth() - 1, 1)), ymd(new Date(now.getFullYear(), now.getMonth(), 0))];
    const a = new Date();
    a.setDate(a.getDate() - 29);
    return [ymd(a), todayStr()];
  });

  $effect(() => {
    const [a, b] = period;
    api.clients(a, b).then((r) => (rows = r));
  });

  const total = $derived(rows.reduce((n, r) => n + r.amount, 0));
  const hours = $derived(rows.reduce((n, r) => n + r.secs, 0));

  async function setRate(r, v) {
    const rate = Number(String(v).replace(/[^\d]/g, '')) || 0;
    st.settings.rates = { ...st.settings.rates, [r.project]: rate };
    await api.saveSettings($state.snapshot(st.settings));
    rows = await api.clients(...period);
  }

  async function csv() {
    try {
      const path = await api.exportClients(...period);
      flash?.(`CSV tersimpan: ${path}`);
      api.reveal(path);
    } catch (e) {
      flash?.(String(e));
    }
  }

  async function invoice(p) {
    busy = p;
    try {
      await api.invoice(p, ...period);
      flash?.('Invoice dibuka di browser — Ctrl+P untuk simpan PDF');
    } catch (e) {
      flash?.(String(e));
    } finally {
      busy = null;
    }
  }

  async function update(p) {
    busy = p;
    try {
      const text = await api.clientUpdate(p);
      await navigator.clipboard.writeText(text);
      flash?.('Update progres disalin ke clipboard');
    } catch (e) {
      flash?.(String(e));
    } finally {
      busy = null;
    }
  }
</script>

<div class="top">
  <div class="seg">
    <span class="thumb" style="width: calc((100% - 6px) / 3); transform: translateX({idx * 100}%)"></span>
    {#each RANGES as r}
      <button class:on={range === r.id} onclick={() => (range = r.id)}>{r.name}</button>
    {/each}
  </div>
  <button class="btn sm" onclick={csv} disabled={!rows.length}>Export CSV</button>
</div>

<div class="kpis">
  <div class="kpi"><span>Jam tercatat</span><b class="num display">{fmtDur(hours)}</b></div>
  <div class="kpi"><span>Bisa ditagih</span><b class="num display accent">{fmtMoney(total, cur)}</b></div>
  <div class="kpi"><span>Klien / project</span><b class="num display">{rows.length}</b></div>
</div>

{#if rows.length}
  <ul class="rows">
    {#each rows as r (r.project)}
      <li>
        <div class="row" role="button" tabindex="0" onclick={() => (open = open === r.project ? null : r.project)} onkeydown={(e) => e.key === 'Enter' && (open = open === r.project ? null : r.project)}>
          <span class="name">{r.project}</span>
          <span class="num muted">{fmtDur(r.secs)}</span>
          <span class="num muted">{r.sessions} sesi</span>
          <span class="num amount" class:zero={!r.amount}>{r.rate ? fmtMoney(r.amount, cur) : 'atur rate'}</span>
        </div>
        {#if open === r.project}
          <div class="detail" transition:slide={{ duration: ms(240) }}>
            <label>
              <span>Rate per jam ({cur})</span>
              <input class="field num" value={r.rate || ''} placeholder="mis. 150000" onchange={(e) => setRate(r, e.currentTarget.value)} />
            </label>
            {#if r.done.length}
              <p class="done">Selesai: {r.done.join(' · ')}</p>
            {/if}
            <div class="acts">
              <button class="btn sm primary" disabled={busy === r.project} onclick={() => invoice(r.project)}>Buat invoice</button>
              <button class="btn sm" disabled={busy === r.project} onclick={() => update(r.project)}>Salin update progres</button>
            </div>
          </div>
        {/if}
      </li>
    {/each}
  </ul>
{:else}
  <p class="empty">Belum ada jam tercatat untuk project di periode ini. Isi kolom project di tugasmu.</p>
{/if}

<style>
  .top {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .kpis {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 10px;
    margin-top: 16px;
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
  .accent {
    color: var(--accent);
  }
  .rows {
    list-style: none;
    margin: 20px 0 0;
    padding: 0;
  }
  .rows li {
    border-bottom: 1px solid var(--line);
  }
  .row {
    display: grid;
    grid-template-columns: 1fr 80px 70px 150px;
    gap: 12px;
    align-items: center;
    padding: 12px 4px;
    font-size: 13.5px;
    cursor: pointer;
  }
  .row:hover .name {
    color: var(--accent);
  }
  .name {
    font-weight: 600;
    transition: color 0.15s var(--ease);
  }
  .muted {
    color: var(--muted);
  }
  .amount {
    text-align: right;
    font-weight: 600;
  }
  .amount.zero {
    color: var(--faint);
    font-weight: 500;
  }
  .detail {
    display: grid;
    gap: 10px;
    padding: 0 4px 14px;
  }
  label {
    display: grid;
    gap: 6px;
    max-width: 240px;
  }
  label span {
    font-size: 12px;
    font-weight: 600;
    color: var(--muted);
  }
  .done {
    margin: 0;
    font-size: 12.5px;
    color: var(--muted);
  }
  .acts {
    display: flex;
    gap: 8px;
  }
  .empty {
    margin-top: 24px;
    color: var(--faint);
    font-size: 13px;
  }
</style>
