<script>
  import { slide, fade } from 'svelte/transition';
  import { quintOut } from 'svelte/easing';
  import { api } from '../lib/api.js';
  import { ms, MODES } from '../lib/util.js';

  let { st, onsaved } = $props();

  const PROVIDERS = [
    { id: 'none', name: 'Tanpa AI' },
    { id: 'claude', name: 'Claude API' },
    { id: 'local', name: 'Lokal' },
    { id: 'custom', name: 'Custom' },
  ];
  const FORMATS = [
    { id: 'openai', name: 'OpenAI-compatible' },
    { id: 'anthropic', name: 'Anthropic-compatible' },
  ];
  const PRESETS = [
    { name: 'LM Studio', url: 'http://localhost:1234/v1' },
    { name: 'Ollama', url: 'http://localhost:11434/v1' },
  ];

  let s = $state(structuredClone($state.snapshot(st.settings)));
  let distractText = $state(toText(st.settings.distractors));
  let keyInput = $state('');
  let hasKey = $state({ claude: st.has_claude_key, local: st.has_local_key, custom: st.has_custom_key });
  const fmtIdx = $derived(Math.max(0, FORMATS.findIndex((f) => f.id === s.custom_format)));
  let toast = $state('');
  let toastErr = $state(false);
  let testing = $state(false);
  let testResult = $state('');
  let models = $state([]);
  let saving = $state(false);

  const provIdx = $derived(PROVIDERS.findIndex((p) => p.id === s.ai_provider));
  const modeIdx = $derived(MODES.findIndex((m) => m.id === s.default_mode));

  function toText(list) {
    return list.map((d) => `${d.name}: ${d.patterns.join(', ')}`).join('\n');
  }
  function fromText(text) {
    return text
      .split('\n')
      .map((l) => l.trim())
      .filter(Boolean)
      .map((l) => {
        const i = l.indexOf(':');
        const name = (i < 0 ? l : l.slice(0, i)).trim();
        const pats = (i < 0 ? l : l.slice(i + 1)).split(',').map((p) => p.trim()).filter(Boolean);
        return { name, patterns: pats.length ? pats : [name.toLowerCase()] };
      });
  }

  let toastTimer;
  function flash(msg, err = false) {
    toast = msg;
    toastErr = err;
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => (toast = ''), 2600);
  }

  async function save() {
    saving = true;
    try {
      await api.saveSettings({ ...s, distractors: fromText(distractText) });
      flash('Tersimpan');
      onsaved?.();
    } catch (e) {
      flash(String(e), true);
    } finally {
      saving = false;
    }
  }

  async function saveKey() {
    const kind = s.ai_provider;
    try {
      await api.setSecret(kind, keyInput);
      hasKey[kind] = !!keyInput.trim();
      keyInput = '';
      flash(hasKey[kind] ? 'Key tersimpan di Windows Credential Manager' : 'Key dihapus');
    } catch (e) {
      flash(String(e), true);
    }
  }

  async function test() {
    testing = true;
    testResult = '';
    try {
      await api.saveSettings({ ...s, distractors: fromText(distractText) });
      testResult = await api.testAi();
    } catch (e) {
      testResult = `Gagal: ${e}`;
    } finally {
      testing = false;
    }
  }

  async function findModels() {
    const custom = s.ai_provider === 'custom';
    try {
      models = await api.localModels(custom ? s.custom_base_url : s.local_base_url, s.ai_provider);
      if (!models.length) flash('Server nyala, tapi belum ada model yang tersedia', true);
      else if (custom && !s.custom_model) s.custom_model = models[0];
      else if (!custom && !s.local_model) s.local_model = models[0];
    } catch (e) {
      models = [];
      flash(String(e), true);
    }
  }
</script>

{#snippet keyField(kind, placeholder)}
  <label>
    <span>API key {#if hasKey[kind]}<em class="ok">tersimpan</em>{/if}</span>
    <div class="inline">
      <input class="field" type="password" placeholder={hasKey[kind] ? '•••••••• (isi untuk ganti)' : placeholder} bind:value={keyInput} />
      <button class="btn" onclick={saveKey}>{keyInput ? 'Simpan key' : hasKey[kind] ? 'Hapus' : 'Simpan key'}</button>
    </div>
  </label>
{/snippet}

<div class="wrap">
  <section>
    <h3>Otak AI</h3>
    <p class="hint">Dipakai untuk ubah satu kalimat jadi tugas + langkah kecil. Tanpa AI tetap jalan pakai parser bawaan.</p>
    <div class="seg prov">
      <span class="thumb" style="width: calc((100% - 6px) / {PROVIDERS.length}); transform: translateX({provIdx * 100}%)"></span>
      {#each PROVIDERS as p}
        <button
          class:on={s.ai_provider === p.id}
          onclick={() => {
            s.ai_provider = p.id;
            keyInput = '';
            models = [];
            testResult = '';
          }}>{p.name}</button
        >
      {/each}
    </div>

    {#key s.ai_provider}
      <div class="sub" in:slide={{ duration: ms(260), easing: quintOut }}>
        {#if s.ai_provider === 'claude'}
          <label>
            <span>Model</span>
            <input class="field" bind:value={s.claude_model} spellcheck="false" />
          </label>
          {@render keyField('claude', 'sk-ant-…')}
        {:else if s.ai_provider === 'custom'}
          <label>
            <span>Format API</span>
            <div class="seg fmt">
              <span class="thumb" style="width: calc((100% - 6px) / 2); transform: translateX({fmtIdx * 100}%)"></span>
              {#each FORMATS as f}
                <button class:on={s.custom_format === f.id} onclick={() => (s.custom_format = f.id)}>{f.name}</button>
              {/each}
            </div>
          </label>
          <label>
            <span>Base URL <em>{s.custom_format === 'anthropic' ? 'mis. https://api.z.ai/api/anthropic' : 'mis. https://openrouter.ai/api/v1'}</em></span>
            <input class="field" bind:value={s.custom_base_url} placeholder="https://…" spellcheck="false" />
          </label>
          {@render keyField('custom', 'API key provider')}
          <label>
            <span>Model {#if s.custom_format === 'openai'}<em>kosong = model pertama dari server</em>{/if}</span>
            <div class="inline">
              {#if models.length}
                <select class="field" bind:value={s.custom_model}>
                  {#each models as m}<option value={m}>{m}</option>{/each}
                </select>
              {:else}
                <input class="field" bind:value={s.custom_model} placeholder="mis. glm-4.6, deepseek-chat" spellcheck="false" />
              {/if}
              {#if s.custom_format === 'openai'}
                <button class="btn" onclick={findModels}>Cari model</button>
              {/if}
            </div>
          </label>
        {:else if s.ai_provider === 'local'}
          <label>
            <span>Server (OpenAI-compatible)</span>
            <div class="inline">
              <input class="field" bind:value={s.local_base_url} spellcheck="false" />
              {#each PRESETS as p}
                <button class="btn sm" class:on={s.local_base_url === p.url} onclick={() => (s.local_base_url = p.url)}>{p.name}</button>
              {/each}
            </div>
          </label>
          <label>
            <span>Model <em>kosong = pakai yang sedang ter-load</em></span>
            <div class="inline">
              {#if models.length}
                <select class="field" bind:value={s.local_model}>
                  {#each models as m}<option value={m}>{m}</option>{/each}
                </select>
              {:else}
                <input class="field" bind:value={s.local_model} placeholder="mis. qwen3.5-9b" spellcheck="false" />
              {/if}
              <button class="btn" onclick={findModels}>Cari model</button>
            </div>
          </label>
        {/if}
        {#if s.ai_provider !== 'none'}
          <div class="inline test">
            <button class="btn" disabled={testing} onclick={test}>{testing ? 'Ngetes…' : 'Tes AI'}</button>
            {#if testResult}<span class="result" class:bad={testResult.startsWith('Gagal')} in:fade>{testResult}</span>{/if}
          </div>
        {/if}
      </div>
    {/key}
  </section>

  <section>
    <h3>Fokus</h3>
    <div class="grid">
      <label>
        <span>Mode default</span>
        <div class="seg">
          <span class="thumb" style="width: calc((100% - 6px) / 3); transform: translateX({modeIdx * 100}%)"></span>
          {#each MODES as m}
            <button class:on={s.default_mode === m.id} title={m.hint} onclick={() => (s.default_mode = m.id)}>{m.name}</button>
          {/each}
        </div>
      </label>
      <label>
        <span>Durasi default (menit)</span>
        <input class="field num" type="number" min="5" max="240" bind:value={s.default_minutes} />
      </label>
      <label>
        <span>Toleransi sebelum ditegur (detik)</span>
        <input class="field num" type="number" min="1" max="120" bind:value={s.grace_secs} />
      </label>
      <label>
        <span>Jarak HUD dari atas layar (px) <em>naikkan kalau nabrak widget lain</em></span>
        <input class="field num" type="number" min="0" max="400" bind:value={s.hud_top} />
      </label>
      <label>
        <span>Laporan harian muncul jam</span>
        <input class="field num" type="number" min="0" max="23" bind:value={s.report_hour} />
      </label>
    </div>
  </section>

  <section>
    <h3>Pengganggu</h3>
    <p class="hint">Satu per baris: <code>Nama: pola, pola</code>. Pola biasa dicocokkan ke judul tab browser, <code>exe:nama</code> ke nama aplikasi.</p>
    <textarea class="field mono" rows="8" spellcheck="false" bind:value={distractText}></textarea>
  </section>

  <section>
    <h3>Sistem</h3>
    <div class="grid">
      <label>
        <span>Hotkey tangkap cepat {#if st.hotkey_error}<em class="bad">{st.hotkey_error}</em>{/if}</span>
        <input class="field" bind:value={s.hotkey} spellcheck="false" />
      </label>
      <label class="toggle">
        <span>Nyala otomatis saat Windows start</span>
        <button class="switch" class:on={s.autostart} role="switch" aria-checked={s.autostart} aria-label="Autostart" onclick={() => (s.autostart = !s.autostart)}><i></i></button>
      </label>
    </div>
    <p class="hint small">Data tersimpan lokal di <code>{st.data_dir}</code></p>
  </section>

  <div class="savebar">
    {#if toast}
      <span class="toast" class:bad={toastErr} transition:fade={{ duration: ms(150) }}>{toast}</span>
    {/if}
    <button class="btn primary" disabled={saving} onclick={save}>Simpan pengaturan</button>
  </div>
</div>

<style>
  .wrap {
    max-width: 760px;
    margin: 0 auto;
    padding: 6px 28px 90px;
  }
  section {
    padding: 22px 0;
    border-bottom: 1px solid var(--line);
  }
  h3 {
    margin: 0 0 4px;
    font-size: 15px;
    font-weight: 650;
  }
  .hint {
    margin: 0 0 14px;
    color: var(--muted);
    font-size: 13px;
  }
  .hint.small {
    margin: 14px 0 0;
    font-size: 12px;
    color: var(--faint);
    user-select: text;
  }
  code {
    font-family: 'Cascadia Code', Consolas, monospace;
    font-size: 12px;
    padding: 1px 5px;
    border-radius: 5px;
    background: var(--surface-2);
  }
  .prov {
    width: 100%;
    max-width: 480px;
  }
  .fmt {
    justify-self: start;
  }
  .sub {
    display: grid;
    gap: 14px;
    margin-top: 16px;
  }
  label {
    display: grid;
    gap: 6px;
  }
  label > span {
    font-size: 12px;
    font-weight: 600;
    color: var(--muted);
  }
  em {
    font-style: normal;
    font-weight: 500;
    margin-left: 6px;
    color: var(--faint);
  }
  em.ok {
    color: var(--accent);
  }
  em.bad,
  .bad {
    color: var(--danger);
  }
  .inline {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .inline .field {
    flex: 1;
  }
  .btn.on {
    background: var(--accent-soft);
    color: var(--accent);
  }
  .test {
    align-items: flex-start;
  }
  .result {
    flex: 1;
    padding-top: 8px;
    font-size: 12.5px;
    color: var(--accent);
    user-select: text;
  }
  .result.bad {
    color: var(--danger);
  }
  .grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 16px 20px;
    margin-top: 12px;
  }
  .grid .seg {
    justify-self: start;
  }
  .mono {
    font-family: 'Cascadia Code', Consolas, monospace;
    font-size: 12.5px;
  }
  .toggle {
    align-content: start;
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
  .savebar {
    position: fixed;
    right: 24px;
    bottom: 20px;
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .toast {
    font-size: 12.5px;
    color: var(--accent);
    padding: 6px 12px;
    border-radius: 999px;
    background: var(--surface-2);
    border: 1px solid var(--line);
  }
  .toast.bad {
    color: var(--danger);
  }
</style>
