<script>
  import { slide, fade } from 'svelte/transition';
  import { quintOut } from 'svelte/easing';
  import { api } from '../lib/api.js';
  import { isTauri } from '../lib/tauri.js';
  import { ms, MODES, baseName } from '../lib/util.js';

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
  const STT = [
    { name: 'Groq', url: 'https://api.groq.com/openai/v1', model: 'whisper-large-v3-turbo' },
    { name: 'OpenAI', url: 'https://api.openai.com/v1', model: 'whisper-1' },
    { name: 'Lokal', url: 'http://localhost:8000/v1', model: 'Systran/faster-whisper-small' },
  ];
  const SECTIONS = [
    ['ai', 'AI'],
    ['fokus', 'Fokus'],
    ['ganggu', 'Pengganggu'],
    ['kerja', 'Mode kerja'],
    ['template', 'Template'],
    ['render', 'Render'],
    ['integrasi', 'Integrasi'],
    ['sistem', 'Sistem'],
  ];

  let s = $state(structuredClone($state.snapshot(st.settings)));
  let distractText = $state(toText(st.settings.distractors));
  let keys = $state({});
  let hasKey = $state({
    claude: st.has_claude_key,
    local: st.has_local_key,
    custom: st.has_custom_key,
    telegram: st.has_telegram_key,
    stt: st.has_stt_key,
  });
  const fmtIdx = $derived(Math.max(0, FORMATS.findIndex((f) => f.id === s.custom_format)));
  let toast = $state('');
  let toastErr = $state(false);
  let testing = $state(false);
  let testResult = $state('');
  let models = $state([]);
  let saving = $state(false);
  let pairCode = $state(s.telegram_pair_code || '');
  let checking = $state(false);

  const provIdx = $derived(PROVIDERS.findIndex((p) => p.id === s.ai_provider));
  const modeIdx = $derived(MODES.findIndex((m) => m.id === s.default_mode));
  const projects = $derived([...new Set(st.tasks.filter((t) => t.project).map((t) => t.project))]);

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
  const csv = (v) => (v ?? []).join(', ');
  const fromCsv = (v) => v.split(',').map((x) => x.trim()).filter(Boolean);
  const lines = (v) => (v ?? []).join('\n');
  const fromLines = (v) => v.split('\n').map((x) => x.trim()).filter(Boolean);

  let toastTimer;
  function flash(msg, err = false) {
    toast = msg;
    toastErr = err;
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => (toast = ''), 2800);
  }

  const payload = () => ({ ...$state.snapshot(s), distractors: fromText(distractText) });

  async function save() {
    saving = true;
    try {
      await api.saveSettings(payload());
      flash('Tersimpan');
      onsaved?.();
    } catch (e) {
      flash(String(e), true);
    } finally {
      saving = false;
    }
  }

  async function saveKey(kind) {
    const v = keys[kind] ?? '';
    try {
      await api.setSecret(kind, v);
      hasKey[kind] = !!v.trim();
      keys[kind] = '';
      flash(hasKey[kind] ? 'Key tersimpan di Windows Credential Manager' : 'Key dihapus');
    } catch (e) {
      flash(String(e), true);
    }
  }

  async function test() {
    testing = true;
    testResult = '';
    try {
      await api.saveSettings(payload());
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

  function addPreset() {
    s.presets = [
      ...s.presets,
      { id: 'p' + Date.now().toString(36), name: 'Mode baru', open: [], close_distractors: true, minutes: s.default_minutes, mode: s.default_mode, project: null },
    ];
  }
  function addTemplate() {
    s.templates = [...s.templates, { name: 'Template baru', keywords: [], steps: [], est_minutes: null }];
  }
  async function addRepo() {
    const dir = isTauri ? await api.pickFolder() : 'D:\\code\\repo';
    if (dir) s.git_repos = [...s.git_repos, { path: dir, project: '' }];
  }
  async function addWatch() {
    const dir = isTauri ? await api.pickFolder() : null;
    if (dir) s.watch_dirs = [...s.watch_dirs, dir];
  }
  async function pickRoot() {
    const dir = isTauri ? await api.pickFolder() : null;
    if (dir) s.export_root = dir;
  }
  async function pickExe(field) {
    const f = isTauri ? await api.pickFile([{ name: 'Program', extensions: ['exe'] }]) : null;
    if (f?.[0]) s[field] = f[0];
  }

  async function exportIcs() {
    const p = await api.exportIcs();
    api.reveal(p);
    flash('gaspol.ics diperbarui — import ke Google Calendar');
  }
  async function refreshCal() {
    try {
      await api.saveSettings(payload());
      const n = await api.refreshCalendar();
      flash(`${n} acara kalender terbaca`);
    } catch (e) {
      flash(String(e), true);
    }
  }
  async function testHook() {
    try {
      await api.testWebhook(s.webhook_url);
      flash('Webhook terkirim');
    } catch (e) {
      flash(String(e), true);
    }
  }
  async function pair() {
    pairCode = await api.telegramPair();
    s.telegram_chat_id = null;
  }
  async function unpair() {
    await api.telegramUnpair();
    s.telegram_chat_id = null;
    pairCode = '';
  }
  async function gitNow() {
    await api.saveSettings(payload());
    const n = await api.gitPoll();
    flash(`${n} commit tercatat hari ini`);
  }
  async function checkUpdate() {
    checking = true;
    try {
      const u = await api.checkUpdate();
      if (!u) flash('Sudah versi terbaru');
      else {
        flash(`Mengunduh ${u.version}…`);
        await u.downloadAndInstall();
        await api.relaunch();
      }
    } catch (e) {
      flash(String(e), true);
    } finally {
      checking = false;
    }
  }

  function jump(id) {
    document.getElementById(id)?.scrollIntoView({ behavior: 'smooth', block: 'start' });
  }
</script>

{#snippet keyField(kind, placeholder, label = 'API key')}
  <label>
    <span>{label} {#if hasKey[kind]}<em class="ok">tersimpan</em>{/if}</span>
    <div class="inline">
      <input class="field" type="password" placeholder={hasKey[kind] ? '•••••••• (isi untuk ganti)' : placeholder} bind:value={keys[kind]} />
      <button class="btn" onclick={() => saveKey(kind)}>{keys[kind] ? 'Simpan' : hasKey[kind] ? 'Hapus' : 'Simpan'}</button>
    </div>
  </label>
{/snippet}

{#snippet toggle(label, key, hint = '')}
  <label class="toggle">
    <span>{label} {#if hint}<em>{hint}</em>{/if}</span>
    <button class="switch" class:on={s[key]} role="switch" aria-checked={s[key]} aria-label={label} onclick={() => (s[key] = !s[key])}><i></i></button>
  </label>
{/snippet}

<div class="wrap">
  <nav class="jump">
    {#each SECTIONS as [id, name]}<button onclick={() => jump(id)}>{name}</button>{/each}
  </nav>

  <section id="ai">
    <h3>Otak AI</h3>
    <p class="hint">Dipakai untuk ubah satu kalimat / screenshot jadi tugas + langkah kecil. Tanpa AI tetap jalan pakai parser bawaan + template.</p>
    <div class="seg prov">
      <span class="thumb" style="width: calc((100% - 6px) / {PROVIDERS.length}); transform: translateX({provIdx * 100}%)"></span>
      {#each PROVIDERS as p}
        <button
          class:on={s.ai_provider === p.id}
          onclick={() => {
            s.ai_provider = p.id;
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

    <h4>Suara → tugas</h4>
    <p class="hint">Tombol mikrofon di capture bar. Endpoint OpenAI-compatible <code>/audio/transcriptions</code> (Whisper).</p>
    <div class="grid">
      <label>
        <span>Server transkripsi</span>
        <div class="inline">
          <input class="field" bind:value={s.stt_base_url} placeholder="https://api.groq.com/openai/v1" spellcheck="false" />
        </div>
        <div class="inline">
          {#each STT as p}
            <button class="btn sm" class:on={s.stt_base_url === p.url} onclick={() => ((s.stt_base_url = p.url), (s.stt_model = p.model))}>{p.name}</button>
          {/each}
        </div>
      </label>
      <label>
        <span>Model</span>
        <input class="field" bind:value={s.stt_model} spellcheck="false" />
      </label>
    </div>
    <div class="grid one">{@render keyField('stt', 'API key server suara (kosong kalau lokal)')}</div>
  </section>

  <section id="fokus">
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
        <span>Ingatkan istirahat tiap (menit aktif) <em>0 = mati</em></span>
        <input class="field num" type="number" min="0" max="600" bind:value={s.break_every_min} />
      </label>
      <label>
        <span>Rencana pagi muncul jam</span>
        <input class="field num" type="number" min="0" max="12" bind:value={s.plan_hour} />
      </label>
      <label>
        <span>Laporan harian muncul jam</span>
        <input class="field num" type="number" min="0" max="23" bind:value={s.report_hour} />
      </label>
      {@render toggle('Pasang niat dulu', 'intent_mode', 'buka pengganggu → pilih berapa menit')}
      {@render toggle('Lindungi flow', 'flow_protect', 'lagi asik → +15 menit otomatis')}
      {@render toggle('Catat otomatis per tugas', 'auto_track', 'jendela cocok dengan tugas → waktunya tercatat')}
    </div>
  </section>

  <section id="ganggu">
    <h3>Pengganggu</h3>
    <p class="hint">Satu per baris: <code>Nama: pola, pola</code>. Pola biasa dicocokkan ke judul tab browser, <code>exe:nama</code> ke nama aplikasi.</p>
    <textarea class="field mono" rows="7" spellcheck="false" bind:value={distractText}></textarea>
    <label class="mt">
      <span>Tetap boleh kalau judul tab berisi <em>pisahkan dengan koma — mis. tutorial Blender di YouTube</em></span>
      <input class="field" value={csv(s.allow_keywords)} onchange={(e) => (s.allow_keywords = fromCsv(e.currentTarget.value))} spellcheck="false" />
    </label>
    <div class="grid">
      {@render toggle('Blokir situs saat mode Galak', 'hard_block', 'lewat file hosts, minta izin admin')}
      <label>
        <span>Musik saat sesi mulai <em>kosong = mati</em></span>
        <div class="inline">
          <input class="field" bind:value={s.music} placeholder="media, spotify:playlist:…, atau URL" spellcheck="false" />
          <button class="btn sm" class:on={s.music === 'media'} onclick={() => (s.music = 'media')}>Tombol media</button>
        </div>
      </label>
    </div>
    {#if s.hard_block}
      <label class="mt" transition:slide={{ duration: ms(200) }}>
        <span>Situs yang diblokir (satu per baris)</span>
        <textarea class="field mono" rows="4" spellcheck="false" value={lines(s.block_sites)} onchange={(e) => (s.block_sites = fromLines(e.currentTarget.value))}></textarea>
      </label>
    {/if}
  </section>

  <section id="kerja">
    <h3>Mode kerja</h3>
    <p class="hint">Satu klik: buka app/folder/URL, tutup pengganggu, mulai sesi. Isi per baris: URL, path folder/file, atau nama app (<code>code</code>, <code>afterfx</code>, <code>blender</code>).</p>
    <div class="cards">
      {#each s.presets as p, i (p.id)}
        <div class="card" transition:slide={{ duration: ms(220) }}>
          <div class="inline">
            <input class="field strong" bind:value={p.name} />
            <button class="btn sm ghost danger" onclick={() => (s.presets = s.presets.filter((_, j) => j !== i))}>Hapus</button>
          </div>
          <textarea class="field mono" rows="3" placeholder="afterfx&#10;D:\Klien\Arunika&#10;https://frame.io" value={lines(p.open)} onchange={(e) => (p.open = fromLines(e.currentTarget.value))}></textarea>
          <div class="inline wrap-row">
            <input class="field num tiny" type="number" min="5" max="240" bind:value={p.minutes} title="Menit" />
            <select class="field" bind:value={p.mode}>
              {#each MODES as m}<option value={m.id}>{m.name}</option>{/each}
            </select>
            <input class="field" list="projects" placeholder="Project (opsional)" value={p.project ?? ''} onchange={(e) => (p.project = e.currentTarget.value.trim() || null)} />
            <label class="check"><input type="checkbox" bind:checked={p.close_distractors} /> tutup pengganggu</label>
          </div>
        </div>
      {/each}
    </div>
    <button class="btn sm" onclick={addPreset}>+ Mode baru</button>
    <datalist id="projects">{#each projects as p}<option value={p}></option>{/each}</datalist>
  </section>

  <section id="template">
    <h3>Template tugas</h3>
    <p class="hint">Kalimat yang mengandung kata kunci template → langkahnya pakai kerangka ini (AI tetap menyesuaikan).</p>
    <div class="cards">
      {#each s.templates as t, i}
        <div class="card" transition:slide={{ duration: ms(220) }}>
          <div class="inline">
            <input class="field strong" bind:value={t.name} />
            <input class="field num tiny" type="number" min="0" placeholder="menit" value={t.est_minutes ?? ''} onchange={(e) => (t.est_minutes = Number(e.currentTarget.value) || null)} title="Estimasi total (menit)" />
            <button class="btn sm ghost danger" onclick={() => (s.templates = s.templates.filter((_, j) => j !== i))}>Hapus</button>
          </div>
          <input class="field" placeholder="kata kunci: teaser, video, reels" value={csv(t.keywords)} onchange={(e) => (t.keywords = fromCsv(e.currentTarget.value))} />
          <textarea class="field mono" rows="4" placeholder="Satu langkah per baris" value={lines(t.steps)} onchange={(e) => (t.steps = fromLines(e.currentTarget.value))}></textarea>
        </div>
      {/each}
    </div>
    <button class="btn sm" onclick={addTemplate}>+ Template baru</button>
  </section>

  <section id="render">
    <h3>Render & export</h3>
    <div class="grid">
      <label>
        <span>blender.exe <em>{st.render?.blender ? 'terdeteksi otomatis' : 'belum ketemu'}</em></span>
        <div class="inline">
          <input class="field" bind:value={s.blender_path} placeholder={st.render?.blender ?? 'C:\\Program Files\\Blender Foundation\\…'} spellcheck="false" />
          <button class="btn sm" onclick={() => pickExe('blender_path')}>Pilih</button>
        </div>
      </label>
      <label>
        <span>aerender.exe <em>{st.render?.aerender ? 'terdeteksi otomatis' : 'belum ketemu'}</em></span>
        <div class="inline">
          <input class="field" bind:value={s.aerender_path} placeholder={st.render?.aerender ?? '…\\Support Files\\aerender.exe'} spellcheck="false" />
          <button class="btn sm" onclick={() => pickExe('aerender_path')}>Pilih</button>
        </div>
      </label>
      {@render toggle('Rapikan export otomatis', 'organize_exports', 'pindah + rename jadi tanggal_nama_vNN')}
      <label>
        <span>Folder tujuan <em>kosong = folder asal</em></span>
        <div class="inline">
          <input class="field" bind:value={s.export_root} placeholder="D:\Klien" spellcheck="false" />
          <button class="btn sm" onclick={pickRoot}>Pilih</button>
        </div>
      </label>
    </div>
    {#if s.organize_exports}
      <div class="list" transition:slide={{ duration: ms(200) }}>
        <span class="lbl">Folder yang dipantau (file video/audio baru)</span>
        {#each s.watch_dirs as d, i}
          <div class="inline"><code class="grow">{d}</code><button class="btn sm ghost" onclick={() => (s.watch_dirs = s.watch_dirs.filter((_, j) => j !== i))}>Hapus</button></div>
        {/each}
        <button class="btn sm" onclick={addWatch}>+ Folder</button>
      </div>
    {/if}
  </section>

  <section id="integrasi">
    <h3>Integrasi</h3>

    <h4>Git</h4>
    <p class="hint">Commit baru → tugas di project yang sama ikut tersentuh, langkah yang mirip pesan commit dicentang.</p>
    <div class="list">
      {#each s.git_repos as r, i}
        <div class="inline">
          <code class="grow" title={r.path}>{baseName(r.path)}</code>
          <input class="field" list="projects" placeholder="Project" bind:value={r.project} />
          <button class="btn sm ghost" onclick={() => (s.git_repos = s.git_repos.filter((_, j) => j !== i))}>Hapus</button>
        </div>
      {/each}
      <div class="inline">
        <button class="btn sm" onclick={addRepo}>+ Repo</button>
        {#if s.git_repos.length}<button class="btn sm ghost" onclick={gitNow}>Cek commit sekarang</button>{/if}
      </div>
    </div>

    <h4>Kalender</h4>
    <p class="hint">
      Tempel <b>alamat rahasia format iCal</b> dari Google Calendar (Setelan kalender → Integrasikan kalender) supaya rencana pagi tahu jam kosongmu.
      Deadline & sesi fokus Gaspol bisa diimpor balik lewat file <code>gaspol.ics</code>.
    </p>
    <div class="inline">
      <input class="field" bind:value={s.ics_url} placeholder="https://calendar.google.com/calendar/ical/…/basic.ics" spellcheck="false" />
      <button class="btn sm" onclick={refreshCal}>Muat</button>
      <button class="btn sm ghost" onclick={exportIcs}>Ekspor .ics</button>
    </div>
    {#if st.cal_error}<p class="hint bad small">{st.cal_error}</p>{:else if st.cal_count}<p class="hint small">{st.cal_count} acara 7 hari ke depan terbaca.</p>{/if}

    <h4>Akuntabilitas</h4>
    <p class="hint">Ringkasan harian dikirim otomatis ke channel Discord (atau Slack) temanmu saat laporan muncul.</p>
    <div class="inline">
      <input class="field" bind:value={s.webhook_url} placeholder="https://discord.com/api/webhooks/…" spellcheck="false" />
      <button class="btn sm" disabled={!s.webhook_url} onclick={testHook}>Tes</button>
    </div>

    <h4>Telegram (HP)</h4>
    <p class="hint">Bikin bot lewat <b>@BotFather</b>, tempel tokennya, lalu kirim kode pairing ke bot. Setelah itu: kirim kalimat apa pun → jadi tugas; <code>/hari</code> <code>/fokus</code> <code>/stop</code> <code>/laporan</code>. Pengingat & render selesai juga dikirim ke sana.</p>
    <div class="grid one">{@render keyField('telegram', '123456:ABC-…', 'Token bot')}</div>
    <div class="inline mt">
      {#if s.telegram_chat_id}
        <span class="ok-t">Terhubung ✓</span>
        <button class="btn sm ghost" onclick={unpair}>Putuskan</button>
      {:else}
        <button class="btn sm" disabled={!hasKey.telegram} onclick={pair}>Buat kode pairing</button>
        {#if pairCode}<span>Kirim <code class="big">{pairCode}</code> ke bot-mu</span>{/if}
      {/if}
      {#if st.telegram_error}<span class="bad small">{st.telegram_error}</span>{/if}
    </div>

    <h4>Klien</h4>
    <div class="grid">
      <label>
        <span>Mata uang</span>
        <input class="field" bind:value={s.currency} />
      </label>
      <p class="hint small">Rate per jam diatur di Laporan → Klien.</p>
    </div>
  </section>

  <section id="sistem">
    <h3>Sistem</h3>
    <div class="grid">
      <label>
        <span>Hotkey tangkap cepat {#if st.hotkey_error}<em class="bad">{st.hotkey_error}</em>{/if}</span>
        <input class="field" bind:value={s.hotkey} spellcheck="false" />
      </label>
      <label>
        <span>Hotkey tangkap teks yang diblok</span>
        <input class="field" bind:value={s.selection_hotkey} spellcheck="false" />
      </label>
      <label>
        <span>HUD muncul di</span>
        <select class="field" bind:value={s.hud_monitor}>
          <option value={0}>Monitor utama</option>
          {#each st.monitors ?? [] as m, i}<option value={i + 1}>{m}</option>{/each}
        </select>
      </label>
      <label>
        <span>Jarak HUD dari atas layar (px)</span>
        <input class="field num" type="number" min="0" max="400" bind:value={s.hud_top} />
      </label>
      {@render toggle('Nyala otomatis saat Windows start', 'autostart')}
      <label>
        <span>Versi {st.version}</span>
        <div class="inline"><button class="btn sm" disabled={checking} onclick={checkUpdate}>{checking ? 'Mengecek…' : 'Cek update'}</button></div>
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
  .jump {
    position: sticky;
    top: 0;
    z-index: 5;
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    padding: 8px 0 10px;
    background: linear-gradient(var(--bg) 75%, transparent);
  }
  .jump button {
    padding: 4px 10px;
    border-radius: 999px;
    font-size: 12px;
    font-weight: 600;
    color: var(--muted);
  }
  .jump button:hover {
    background: var(--surface-2);
    color: var(--text);
  }
  section {
    padding: 22px 0;
    border-bottom: 1px solid var(--line);
    scroll-margin-top: 44px;
  }
  h3 {
    margin: 0 0 4px;
    font-size: 15px;
    font-weight: 650;
  }
  h4 {
    margin: 22px 0 4px;
    font-size: 13px;
    font-weight: 650;
  }
  .hint {
    margin: 0 0 12px;
    color: var(--muted);
    font-size: 13px;
  }
  .hint.small,
  .small {
    font-size: 12px;
  }
  .hint.small {
    margin: 10px 0 0;
    color: var(--faint);
    user-select: text;
  }
  code {
    font-family: 'Cascadia Code', Consolas, monospace;
    font-size: 12px;
    padding: 1px 5px;
    border-radius: 5px;
    background: var(--surface-2);
    user-select: text;
  }
  code.big {
    font-size: 15px;
    letter-spacing: 0.1em;
    color: var(--accent);
  }
  code.grow {
    flex: 1;
    padding: 7px 10px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
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
    align-content: start;
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
  em.ok,
  .ok-t {
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
    min-width: 0;
  }
  .wrap-row {
    flex-wrap: wrap;
  }
  .inline .tiny,
  .tiny {
    flex: none;
    width: 76px;
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
  .grid.one {
    grid-template-columns: 1fr;
  }
  .grid .seg {
    justify-self: start;
  }
  .mt {
    margin-top: 14px;
  }
  .mono {
    font-family: 'Cascadia Code', Consolas, monospace;
    font-size: 12.5px;
  }
  .cards {
    display: grid;
    gap: 10px;
    margin-bottom: 10px;
  }
  .card {
    display: grid;
    gap: 8px;
    padding: 12px;
    border-radius: var(--r);
    background: var(--surface);
    border: 1px solid var(--line);
  }
  .strong {
    font-weight: 600;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12.5px;
    color: var(--muted);
    white-space: nowrap;
  }
  .list {
    display: grid;
    gap: 6px;
    margin-top: 12px;
  }
  .lbl {
    font-size: 12px;
    font-weight: 600;
    color: var(--muted);
  }
  .danger {
    color: var(--danger);
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
    z-index: 6;
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
