<script>
  // Tranche d'une source : marche/arrêt et coupure à gauche, nom, état et vumètre au
  // centre, volume à droite.
  import Knob from "./Knob.svelte";
  import Meter from "./Meter.svelte";
  let { source, t, level = [0, 0], onchange } = $props();

  const toDb = (g) => (g > 0 ? Math.max(20 * Math.log10(g), -40) : -40);

  const status = $derived(
    !source.enabled
      ? { key: "stDisabled", led: "idle" }
      : source.muted
        ? { key: "stMuted", led: "warn" }
        : !source.active
          ? { key: "stPaused", led: "idle" }
          : source.playing
            ? { key: "stPlaying", led: "ok" }
            : { key: "stPriming", led: "idle" },
  );

  function send(patch) {
    onchange(source.name, {
      enabled: source.enabled,
      gain: source.gain,
      muted: source.muted,
      ...patch,
    });
  }
  const setDb = (db) => send({ gain: db <= -40 ? 0 : Math.pow(10, db / 20) });

  const dropouts = $derived(source.underruns + source.resyncs);
  const details = $derived(
    t("details", {
      fill: t.num(source.fill_ms, 1),
      target: t.num(source.target_ms, 0),
      ppm: `${source.correction_ppm >= 0 ? "+" : "−"}${Math.abs(source.correction_ppm).toFixed(0)}`,
    }),
  );
</script>

<article class="strip" class:off={!source.enabled}>
  <div class="buttons">
    <button
      class="power"
      aria-pressed={source.enabled}
      aria-label={t("enable")}
      title={t("enable")}
      onclick={() => send({ enabled: !source.enabled })}
    >
      <svg width="14" height="14" viewBox="0 0 24 24" aria-hidden="true">
        <path d="M12 3v8M6.3 6.8a8 8 0 1 0 11.4 0" />
      </svg>
    </button>
    {#if source.enabled}
      <button
        class="mute mono"
        aria-pressed={source.muted}
        aria-label={source.muted ? t("unmute") : t("mute")}
        title={source.muted ? t("unmute") : t("mute")}
        onclick={() => send({ muted: !source.muted })}>M</button
      >
    {/if}
  </div>

  <div class="body">
    <div class="top">
      <strong title={source.name}>{source.name}</strong>
      <span class="state" title={status.key === "stPaused" ? t("stPausedHint") : ""}>
        <span class="led {status.led}"></span>
        <span class="label {status.led}">{t(status.key)}</span>
      </span>
    </div>
    <span class="fmt">
      {t.num(source.sample_rate / 1000, source.sample_rate % 1000 ? 1 : 0)} kHz · {source.channels === 1 ? t("mono") : t("stereo")}
      {#if source.anti_alias}· {t("antiAlias")}{/if}
    </span>
    {#if source.enabled}
      <Meter {level} dim={source.muted} />
      {#if source.playing}
        <span class="details mono" title={details}>
          {details}{#if dropouts}<span class="drop"> · {t("dropouts", { n: dropouts })}</span>{/if}
        </span>
      {/if}
    {/if}
  </div>

  {#if source.enabled}
    <Knob
      value={toDb(source.gain)}
      label={`${t("volume")} — ${source.name}`}
      format={(v) => t.num(v, 1)}
      hint={t("knobHint")}
      onchange={setDb}
    />
  {/if}
</article>

<style>
  .strip {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    padding: 12px;
    background: var(--face);
    border: 1px solid var(--line);
    border-radius: 10px;
  }
  .strip.off {
    align-items: center;
    opacity: 0.75;
  }
  .buttons {
    display: grid;
    gap: 8px;
  }
  .buttons button {
    width: 28px;
    height: 28px;
    padding: 0;
    display: grid;
    place-items: center;
  }
  .power {
    border-radius: 50%;
  }
  .power path {
    fill: none;
    stroke: var(--dim);
    stroke-width: 2.4;
    stroke-linecap: round;
  }
  .power[aria-pressed="true"] {
    border-color: var(--amber);
  }
  .power[aria-pressed="true"] path {
    stroke: var(--amber);
  }
  .mute {
    font-size: 12px;
    font-weight: 500;
    color: var(--muted);
  }
  .mute[aria-pressed="true"] {
    background: var(--bad);
    border-color: var(--bad);
    color: var(--ink);
  }
  .body {
    flex: 1;
    min-width: 0;
    display: grid;
    gap: 6px;
  }
  .top {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  strong {
    flex: 1;
    min-width: 0;
    font-size: 15px;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .state {
    display: flex;
    align-items: center;
    gap: 6px;
    flex: none;
  }
  .label.ok {
    color: var(--ok);
  }
  .label.warn {
    color: var(--warn);
  }
  .fmt {
    font-size: 12px;
    color: var(--muted);
  }
  .drop {
    color: var(--warn);
  }
  .details {
    font-size: 10.5px;
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
