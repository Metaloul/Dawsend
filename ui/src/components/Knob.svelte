<script>
  // Bouton rotatif de volume, de `min` à `max` dB sur 270°.
  // Souris : glisser vers le haut ou le bas (Maj : réglage fin), molette ; double-clic : 0 dB.
  // Clavier : flèches (0,5 dB), Page haut/bas (3 dB), Début/Fin.
  // Pendant un geste, l'appel au cœur part au plus une fois par image.
  let { value, min = -40, max = 6, size = 44, label, hint = "", format = (v) => v.toFixed(1), onchange } = $props();

  let local = $state(null);
  let el = $state();
  const shown = $derived(local ?? value);
  const frac = $derived((Math.min(Math.max(shown, min), max) - min) / (max - min));
  const text = $derived(
    shown <= min ? "−∞ dB" : `${shown > 0 ? "+" : shown < 0 ? "−" : ""}${format(Math.abs(shown))} dB`,
  );

  // Géométrie : arc de -135° à +135°, angle compté depuis midi dans le sens horaire.
  const c = $derived(size / 2);
  const r = $derived(c - 3);
  const cap = $derived(r - 5);
  const pt = (rad, deg) => {
    const a = (deg * Math.PI) / 180;
    return [c + rad * Math.sin(a), c - rad * Math.cos(a)];
  };
  const arc = (a0, a1) => {
    const [x0, y0] = pt(r, a0);
    const [x1, y1] = pt(r, a1);
    return `M${x0.toFixed(2)} ${y0.toFixed(2)}A${r} ${r} 0 ${a1 - a0 > 180 ? 1 : 0} 1 ${x1.toFixed(2)} ${y1.toFixed(2)}`;
  };
  const angle = $derived(-135 + 270 * frac);
  const tip = $derived(pt(cap - 3, angle));
  const base = $derived(pt(cap * 0.35, angle));

  const clamp = (v) => Math.min(Math.max(Math.round(v * 2) / 2, min), max);

  let pending = null;
  let raf = 0;
  let release = 0;
  function emit(v) {
    local = v;
    pending = v;
    clearTimeout(release);
    if (!raf) {
      raf = requestAnimationFrame(() => {
        raf = 0;
        if (pending !== null) onchange(pending);
        pending = null;
      });
    }
  }
  // Garde la valeur locale le temps que l'état du cœur revienne (rafraîchi toutes les 100 ms).
  function settle() {
    clearTimeout(release);
    release = setTimeout(() => (local = null), 400);
  }

  let drag = null;
  function down(e) {
    if (e.button !== 0) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    e.currentTarget.focus();
    drag = { y: e.clientY, v: shown };
  }
  function move(e) {
    if (!drag) return;
    const travel = e.shiftKey ? 800 : 200;
    emit(clamp(drag.v + ((drag.y - e.clientY) * (max - min)) / travel));
  }
  function up() {
    if (!drag) return;
    drag = null;
    settle();
  }
  function key(e) {
    const steps = { ArrowUp: 0.5, ArrowRight: 0.5, ArrowDown: -0.5, ArrowLeft: -0.5, PageUp: 3, PageDown: -3 };
    let v;
    if (e.key in steps) v = shown + steps[e.key];
    else if (e.key === "Home") v = min;
    else if (e.key === "End") v = max;
    else return;
    e.preventDefault();
    emit(clamp(v));
    settle();
  }
  function reset() {
    emit(0);
    settle();
  }

  // Molette : écouteur non passif pour empêcher le défilement de la fenêtre.
  $effect(() => {
    if (!el) return;
    const wheel = (e) => {
      e.preventDefault();
      emit(clamp(shown + (e.deltaY < 0 ? 0.5 : -0.5)));
      settle();
    };
    el.addEventListener("wheel", wheel, { passive: false });
    return () => el.removeEventListener("wheel", wheel);
  });
</script>

<div class="knob" style="--size: {size}px">
  <svg
    bind:this={el}
    width={size}
    height={size}
    viewBox="0 0 {size} {size}"
    role="slider"
    tabindex="0"
    aria-label={label}
    aria-valuemin={min}
    aria-valuemax={max}
    aria-valuenow={shown}
    aria-valuetext={text}
    title={hint}
    onpointerdown={down}
    onpointermove={move}
    onpointerup={up}
    onpointercancel={up}
    onkeydown={key}
    ondblclick={reset}
  >
    <defs>
      <linearGradient id="knob-cap" x1="0" y1="0" x2="0" y2="1">
        <stop offset="0" stop-color="#3a3f48" />
        <stop offset="1" stop-color="#1f2329" />
      </linearGradient>
    </defs>
    <path d={arc(-135, 135)} class="track" />
    {#if frac > 0.002}<path d={arc(-135, angle)} class="value" />{/if}
    <circle cx={c} cy={c} r={cap} fill="url(#knob-cap)" stroke="var(--well)" />
    <circle cx={c} cy={c - 0.5} r={cap - 1} class="rim" />
    <line x1={base[0]} y1={base[1]} x2={tip[0]} y2={tip[1]} class="pointer" />
  </svg>
  <span class="mono readout">{text}</span>
</div>

<style>
  .knob {
    display: grid;
    justify-items: center;
    gap: 4px;
    flex: none;
  }
  svg {
    display: block;
    cursor: ns-resize;
    touch-action: none;
    border-radius: 50%;
  }
  path {
    fill: none;
    stroke-width: 3;
    stroke-linecap: round;
  }
  .track {
    stroke: var(--line);
  }
  .value {
    stroke: var(--amber);
  }
  .rim {
    fill: none;
    stroke: #fff;
    stroke-opacity: 0.06;
  }
  .pointer {
    stroke: var(--paper);
    stroke-width: 2;
    stroke-linecap: round;
  }
  .readout {
    font-size: 11px;
    color: var(--paper);
    white-space: nowrap;
  }
</style>
