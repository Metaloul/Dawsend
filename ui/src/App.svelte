<script>
  import { onMount } from "svelte";
  import { api, log } from "./lib/api.js";
  import { translator } from "./lib/i18n.js";
  import SourceCard from "./components/SourceCard.svelte";
  import Meter from "./components/Meter.svelte";
  import Knob from "./components/Knob.svelte";
  import { meters, startMeters } from "./lib/meters.svelte.js";

  let st = $state(null);
  let devices = $state([]);
  let autostart = $state(false);
  let tab = $state("sources");
  let latencyDraft = $state(null);

  const t = $derived(translator(st?.language ?? "fr"));
  const view = $derived(st?.view);
  const settings = $derived(st?.settings);
  const toDb = (g) => (g > 0 ? Math.max(20 * Math.log10(g), -40) : -40);

  async function refresh() {
    if (document.hidden) return;
    try {
      st = await api.state();
    } catch {
      /* déjà journalisé */
    }
  }
  async function loadDevices() {
    try {
      devices = await api.devices();
    } catch {
      devices = [];
    }
  }
  const loadAutostart = () =>
    api
      .getAutostart()
      .then((v) => (autostart = v))
      .catch(() => {});

  onMount(() => {
    log.info("interface chargée");
    refresh();
    loadDevices();
    loadAutostart();
    const timer = setInterval(refresh, 100);
    const stopMeters = startMeters();
    const devTimer = setInterval(() => !document.hidden && loadDevices(), 5000);
    const onVis = () => {
      if (!document.hidden) {
        refresh();
        loadDevices();
        loadAutostart();
      }
    };
    document.addEventListener("visibilitychange", onVis);
    return () => {
      clearInterval(timer);
      stopMeters();
      clearInterval(devTimer);
      document.removeEventListener("visibilitychange", onVis);
    };
  });

  const setSource = (name, s) => api.setSource(name, s).then(refresh).catch(() => {});
  const setDevice = (e) => {
    const v = e.currentTarget.value;
    api
      .setDevice(v === "" ? null : v)
      .then(refresh)
      .catch(() => {});
  };
  function setMaster(db, muted) {
    const gain = db <= -40 ? 0 : Math.pow(10, db / 20);
    api
      .setMaster(gain, muted)
      .then(refresh)
      .catch(() => {});
  }
  const setPrefs = (patch) =>
    api
      .setPrefs({ ...st.prefs, ...patch })
      .then(refresh)
      .catch(() => {});
  const toggleAutostart = (e) => {
    const on = e.currentTarget.checked;
    api
      .setAutostart(on)
      .then(() => (autostart = on))
      .catch(() => (autostart = !on));
  };

  // Marge réellement visée + rééchantillonneur + environ 12 ms de WASAPI partagé.
  const totalLatency = $derived(
    settings && view
      ? Math.round(
          Math.max(
            settings.latency_ms,
            ...view.sources.filter((s) => s.playing).map((s) => s.target_ms),
          ) +
            view.resampler_delay_ms +
            12,
        )
      : 0,
  );
  const raised = $derived(
    view && settings
      ? view.sources.find((s) => s.playing && s.target_ms > settings.latency_ms + 0.5)
      : null,
  );
  const status = $derived(
    !view
      ? null
      : view.output
        ? { led: "ok", text: t("statusOn"), title: "" }
        : view.output_error
          ? { led: "bad", text: t("outputError"), title: view.output_error }
          : { led: "idle", text: t("statusIdle"), title: t("outputIdle") },
  );
  const latency = $derived(latencyDraft ?? settings?.latency_ms ?? 10);

  // Mises à jour : l'état vient du cœur (get_state), l'installation se lance d'ici.
  const upd = $derived(st?.update);
  let updError = $state(null);
  let installing = $state(false);
  async function checkUpdates() {
    updError = null;
    await api.updateCheck().catch(() => {});
    refresh();
  }
  async function installUpdate() {
    updError = null;
    installing = true;
    log.info(`mise à jour ${upd?.available?.version} : installation demandée`);
    try {
      await api.updateInstall();
    } catch (e) {
      updError = String(e) === "daw_open" ? t("updDawOpen") : String(e);
    }
    installing = false;
    refresh();
  }
  const progress = (d) => (d?.[1] ? `${Math.round((d[0] * 100) / d[1])} %` : `${(d[0] / 1e6).toFixed(1)} Mo`);
  const updStatus = $derived(
    !upd?.enabled
      ? t("updDisabled")
      : upd.error
        ? t("updError", { e: upd.error })
        : upd.available
          ? t("updAvailable", { v: upd.available.version })
          : upd.last_check
            ? t("updUpToDate", { time: t.time(upd.last_check) })
            : t("updNever"),
  );
</script>

<div class="window">
  <header class="top">
    <img src="/logo-mark.svg" alt="" class="mark" />
    <img src="/wordmark.svg" alt="Dawsend" class="word" />
    <span class="spacer"></span>
    {#if status}
      <span class="status" title={status.title}>
        <span class="led {status.led}"></span>{status.text}
      </span>
    {/if}
    <button
      class="gh"
      title={t("github")}
      aria-label={t("github")}
      onclick={() => api.openRepo().catch(() => {})}
    >
      <!-- Marque GitHub (Octicons, licence MIT) -->
      <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
        <path
          fill="currentColor"
          d="M8 0c4.42 0 8 3.58 8 8a8.013 8.013 0 0 1-5.45 7.59c-.4.08-.55-.17-.55-.38 0-.27.01-1.13.01-2.2 0-.75-.25-1.23-.54-1.48 1.78-.2 3.65-.88 3.65-3.95 0-.88-.31-1.59-.82-2.15.08-.2.36-1.02-.08-2.12 0 0-.67-.22-2.2.82-.64-.18-1.32-.27-2-.27-.68 0-1.36.09-2 .27-1.53-1.03-2.2-.82-2.2-.82-.44 1.1-.16 1.92-.08 2.12-.51.56-.82 1.28-.82 2.15 0 3.06 1.86 3.75 3.64 3.95-.23.2-.44.55-.51 1.07-.46.21-1.61.55-2.33-.66-.15-.24-.6-.83-1.23-.82-.67.01-.27.38.01.53.34.19.73.9.82 1.13.16.45.68 1.31 2.69.94 0 .67.01 1.3.01 1.49 0 .21-.15.45-.55.38A7.995 7.995 0 0 1 0 8c0-4.42 3.58-8 8-8Z"
        />
      </svg>
    </button>
  </header>

  <main>
    {#if upd?.available}
      <section class="update">
        <div class="upd-line">
          <span class="led ok"></span>
          <strong>{t("updAvailable", { v: upd.available.version })}</strong>
          {#if upd.downloading}
            <span class="mono pct">{t("updDownloading", { pct: progress(upd.downloading) })}</span>
          {:else}
            <button class="primary" disabled={installing} onclick={installUpdate}>{t("updInstall")}</button>
          {/if}
        </div>
        <p class="hint" class:warn={updError}>{updError ?? t("updUac")}</p>
        {#if upd.available.notes}
          <details>
            <summary>{t("updNotes")}</summary>
            <p class="notes">{upd.available.notes}</p>
          </details>
        {/if}
      </section>
    {/if}

    {#if st && st.engine_error}
      <p class="banner">{t("engineError")} : {st.engine_error}</p>
    {/if}

    {#if st && settings && view}
      {#if tab === "sources"}
        <div class="title">
          <span class="label">{t("tabSources")}</span>
          {#if view.sources.length}<span class="mono count">{view.sources.length}</span>{/if}
        </div>
        {#if view.sources.length === 0}
          <section class="empty">
            <img src="/logo-mark.svg" alt="" />
            <h2>{t("noSources")}</h2>
            <p>{t("noSourcesHint")}</p>
          </section>
        {:else}
          <section class="list">
            {#each view.sources as source (source.slot + ":" + source.name)}
              <SourceCard {source} {t} level={meters.sources[source.slot]} onchange={setSource} />
            {/each}
          </section>
        {/if}
      {:else}
        <div class="title"><span class="label">{t("tabSettings")}</span></div>
        <section class="settings">
          <div class="group">
            <div class="row">
              <div class="line">
                <label for="device">{t("output")}</label>
                <select id="device" class="wide" value={settings.device ?? ""} onchange={setDevice}>
                  <option value="">{t("defaultOutput")}</option>
                  {#each devices as d (d.name)}
                    <option value={d.name}>{d.name}{d.is_default ? " *" : ""}</option>
                  {/each}
                  {#if settings.device && !devices.some((d) => d.name === settings.device)}
                    <option value={settings.device}>{settings.device}</option>
                  {/if}
                </select>
              </div>
              <p class="hint">{t("outputHint")}</p>
            </div>
            <div class="row">
              <div class="line">
                <label for="latency">{t("latency")}</label>
                <span class="slider">
                  <input
                    id="latency"
                    type="range"
                    min="3"
                    max="50"
                    step="1"
                    value={latency}
                    style="--fill: {((latency - 3) / 47) * 100}%"
                    oninput={(e) => (latencyDraft = Number(e.currentTarget.value))}
                    onchange={(e) => {
                      api
                        .setLatency(Number(e.currentTarget.value))
                        .then(refresh)
                        .catch(() => {});
                      latencyDraft = null;
                    }}
                  />
                  <span class="mono">{latency} ms</span>
                </span>
              </div>
              <p class="hint">{t("latencyHint", { total: totalLatency })}</p>
              {#if raised}
                <p class="hint warn">
                  {t("latencyAuto", { ms: raised.target_ms.toFixed(0), name: raised.name })}
                </p>
              {/if}
            </div>
          </div>

          <div class="group">
            <div class="row">
              <div class="line">
                <label for="lang">{t("language")}</label>
                <select
                  id="lang"
                  value={st.prefs.language}
                  onchange={(e) => setPrefs({ language: e.currentTarget.value })}
                >
                  <option value="auto">{t("langAuto")}</option>
                  <option value="fr">Français</option>
                  <option value="en">English</option>
                </select>
              </div>
            </div>
            <div class="row">
              <div class="line">
                <label for="autostart">{t("autostart")}</label>
                <span class="switch">
                  <input id="autostart" type="checkbox" checked={autostart} onchange={toggleAutostart} />
                  <span></span>
                </span>
              </div>
              <p class="hint">{t("autostartHint")}</p>
            </div>
            <div class="row">
              <div class="line">
                <label for="openwin">{t("openWindow")}</label>
                <span class="switch">
                  <input
                    id="openwin"
                    type="checkbox"
                    checked={st.prefs.open_window_on_start}
                    onchange={(e) => setPrefs({ open_window_on_start: e.currentTarget.checked })}
                  />
                  <span></span>
                </span>
              </div>
              <p class="hint">{t("closeHint")}</p>
            </div>
          </div>

          <div class="group">
            <div class="row">
              <div class="line">
                <span>{t("updVersion")}</span>
                <span class="ver">
                  <span class="mono">v{st.version}</span>
                  <button disabled={!upd?.enabled || upd?.checking} onclick={checkUpdates}>
                    {upd?.checking ? t("updChecking") : t("updCheck")}
                  </button>
                </span>
              </div>
              <p class="hint" class:warn={upd?.error}>{updStatus}</p>
            </div>
            <div class="row">
              <div class="line">
                <label for="updauto">{t("updAuto")}</label>
                <span class="switch">
                  <input
                    id="updauto"
                    type="checkbox"
                    checked={st.prefs.check_updates}
                    disabled={!upd?.enabled}
                    onchange={(e) => setPrefs({ check_updates: e.currentTarget.checked })}
                  />
                  <span></span>
                </span>
              </div>
              <p class="hint">{t("updAutoHint")}</p>
            </div>
          </div>

          <div class="group">
            <div class="row">
              <div class="line">
                <label for="verbose">{t("verbose")}</label>
                <span class="switch">
                  <input
                    id="verbose"
                    type="checkbox"
                    checked={st.prefs.verbose_log}
                    onchange={(e) => setPrefs({ verbose_log: e.currentTarget.checked })}
                  />
                  <span></span>
                </span>
              </div>
              <p class="hint">{t("verboseHint")}</p>
            </div>
            <div class="row">
              <div class="line">
                <span>{t("logs")}</span>
                <button onclick={() => api.openLogs().catch(() => {})}>{t("openLogs")}</button>
              </div>
            </div>
          </div>
        </section>
      {/if}
    {/if}
  </main>

  {#if st && settings && view && tab === "sources"}
    <section class="master">
      <div class="knob">
        <span class="label">{t("secMaster")}</span>
        <Knob
          size={56}
          value={toDb(settings.master_gain)}
          label={t("master")}
          hint={t("knobHint")}
          format={(v) => t.num(v, 1)}
          onchange={(db) => setMaster(db, settings.master_muted)}
        />
      </div>
      <div class="out">
        <span class="label">{t("secOutput")}</span>
        <strong title={view.output?.device_name ?? status?.title}>
          {view.output ? view.output.device_name : status?.text}
        </strong>
        {#if view.output}
          <span class="mono sub">
            {t.num(view.output.sample_rate / 1000, view.output.sample_rate % 1000 ? 1 : 0)} kHz ·
            {t("latencyShort", { ms: totalLatency })}
          </span>
        {/if}
      </div>
      <button
        class="cut"
        aria-pressed={settings.master_muted}
        onclick={() => setMaster(toDb(settings.master_gain), !settings.master_muted)}
      >
        {settings.master_muted ? t("unmute") : t("mute")}
      </button>
      <div class="vu"><Meter vertical level={meters.master} dim={settings.master_muted} /></div>
    </section>
  {/if}

  <footer>
    <nav>
      <button class:sel={tab === "sources"} aria-pressed={tab === "sources"} onclick={() => (tab = "sources")}>
        {t("tabSources")}
      </button>
      <button class:sel={tab === "settings"} aria-pressed={tab === "settings"} onclick={() => (tab = "settings")}>
        {t("tabSettings")}
      </button>
    </nav>
    {#if st}<span class="mono version" title={t("version", { v: st.version })}>v{st.version}</span>{/if}
  </footer>
</div>

<style>
  .window {
    height: 100%;
    display: grid;
    grid-template-rows: auto 1fr auto auto;
  }

  .top {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 14px 16px;
    background: var(--face);
    border-bottom: 1px solid var(--line);
  }
  .mark {
    width: 26px;
    height: 26px;
  }
  .word {
    height: 16px;
  }
  .spacer {
    flex: 1;
  }
  .gh {
    display: grid;
    place-items: center;
    flex: none;
    width: 28px;
    height: 28px;
    padding: 0;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: var(--muted);
    cursor: pointer;
  }
  .gh:hover {
    color: var(--paper);
    background: var(--ctrl);
  }
  .gh:focus-visible {
    outline: 2px solid var(--amber);
    outline-offset: 1px;
  }
  .status {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    font-weight: 500;
    min-width: 0;
    white-space: nowrap;
  }

  main {
    overflow-y: auto;
    padding: 0 12px 12px;
    scrollbar-width: thin;
    scrollbar-color: var(--ctrl) transparent;
  }
  .title {
    display: flex;
    align-items: center;
    padding: 16px 4px 10px;
  }
  .count {
    margin-left: auto;
    font-size: 11px;
    color: var(--muted);
  }
  .list {
    display: grid;
    gap: 8px;
  }
  .banner {
    margin: 12px 0 0;
    padding: 10px 12px;
    border-radius: 8px;
    background: color-mix(in srgb, var(--bad) 16%, transparent);
    color: var(--bad);
  }

  .empty {
    text-align: center;
    padding: 36px 20px;
    color: var(--muted);
  }
  .empty img {
    width: 44px;
    opacity: 0.6;
  }
  .empty h2 {
    color: var(--paper);
    font-size: 16px;
    font-weight: 600;
    margin: 12px 0 6px;
  }
  .empty p {
    margin: 0;
    line-height: 1.5;
  }

  .update {
    margin-top: 12px;
    display: grid;
    gap: 8px;
    padding: 12px 14px;
    background: var(--face);
    border: 1px solid color-mix(in srgb, var(--amber) 55%, var(--line));
    border-radius: 10px;
  }
  .upd-line {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .upd-line strong {
    flex: 1;
    min-width: 0;
    font-weight: 600;
  }
  .pct {
    font-size: 12px;
    color: var(--muted);
  }
  button.primary {
    background: var(--amber);
    border-color: var(--amber);
    color: var(--ink);
    font-weight: 600;
  }
  details summary {
    cursor: pointer;
    font-size: 12px;
    color: var(--muted);
  }
  .notes {
    margin: 6px 0 0;
    white-space: pre-line;
    font-size: 12px;
    line-height: 1.5;
  }
  .ver {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .ver .mono {
    font-size: 12px;
    color: var(--muted);
  }

  .settings {
    display: grid;
    gap: 8px;
  }
  .group {
    background: var(--face);
    border: 1px solid var(--line);
    border-radius: 10px;
  }
  .row {
    display: grid;
    gap: 8px;
    padding: 10px 14px;
  }
  .row + .row {
    border-top: 1px solid var(--line);
  }
  .line {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    min-height: 30px;
  }
  .line > label,
  .line > span:first-child {
    font-weight: 500;
  }
  .line select {
    width: 180px;
    min-width: 0;
  }
  .line select.wide {
    width: 236px;
  }
  .slider {
    display: flex;
    align-items: center;
    gap: 10px;
    flex: 1;
    max-width: 260px;
  }
  .slider input {
    flex: 1;
  }
  .slider .mono {
    width: 44px;
    text-align: right;
    font-size: 12px;
  }
  .hint {
    margin: 0;
    font-size: 11.5px;
    line-height: 1.4;
    color: var(--muted);
  }
  .hint.warn {
    color: var(--warn);
  }

  .master {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 12px 16px;
    background: var(--face);
    border-top: 1px solid var(--line);
  }
  .knob {
    display: grid;
    justify-items: center;
    gap: 4px;
  }
  .out {
    flex: 1;
    min-width: 0;
    display: grid;
    gap: 4px;
  }
  .out strong {
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sub {
    font-size: 10.5px;
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .cut {
    font-weight: 600;
    min-width: 76px;
  }
  .cut[aria-pressed="true"] {
    background: var(--bad);
    border-color: var(--bad);
    color: var(--ink);
  }
  .vu {
    height: 56px;
  }

  footer {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 8px 12px;
    background: var(--well);
  }
  nav {
    display: flex;
    gap: 4px;
  }
  nav button {
    border: none;
    background: transparent;
    padding: 5px 10px;
    border-radius: 5px;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--muted);
  }
  nav button:hover {
    color: var(--paper);
  }
  nav button.sel {
    background: var(--ctrl);
    color: var(--paper);
  }
  .version {
    margin-left: auto;
    font-size: 10.5px;
    color: var(--muted);
  }
</style>
