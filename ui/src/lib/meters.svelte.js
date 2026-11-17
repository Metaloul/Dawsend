// Vumètres de l'appli : les crêtes sont lues à chaque image (commande get_meters) et la
// retombée de 24 dB/s est calculée image par image, comme dans le plugin.
// Un seul appel en vol à la fois ; le navigateur suspend la boucle quand la fenêtre
// est cachée.
import { invoke } from "@tauri-apps/api/core";
import { log } from "./api.js";

export const meters = $state({ master: [0, 0], sources: {} });

const SILENT = 1e-4;
let raf = 0;
let last = 0;
let inflight = false;
let pending = null;
let failing = false;

const decay = (old = [0, 0], peak, fall) =>
  [0, 1].map((c) => Math.max(peak?.[c] ?? 0, old[c] * fall));

function merge(a, b) {
  if (!a) return b;
  const sources = new Map(a.sources.map((s) => [s.slot, s.peak]));
  for (const s of b.sources) sources.set(s.slot, decay(sources.get(s.slot), s.peak, 1));
  return {
    master: decay(a.master, b.master, 1),
    sources: [...sources].map(([slot, peak]) => ({ slot, peak })),
  };
}

function fetchMeters() {
  if (inflight) return;
  inflight = true;
  invoke("get_meters")
    .then((m) => {
      pending = merge(pending, m);
      if (failing) log.info("vumètres : lecture rétablie");
      failing = false;
    })
    .catch((e) => {
      // Moteur arrêté : on le note une fois, sans remplir le journal 60 fois par seconde.
      if (!failing) log.error(`vumètres : lecture impossible : ${e}`);
      failing = true;
    })
    .finally(() => (inflight = false));
}

function frame(now) {
  raf = requestAnimationFrame(frame);
  const dt = last ? Math.min((now - last) / 1000, 0.1) : 0;
  last = now;
  const fall = Math.pow(10, (-24 * dt) / 20);
  const got = pending;
  pending = null;

  meters.master = decay(meters.master, got?.master, fall);
  const fresh = new Map(got?.sources.map((s) => [s.slot, s.peak]) ?? []);
  const next = {};
  for (const slot of new Set([...Object.keys(meters.sources).map(Number), ...fresh.keys()])) {
    const v = decay(meters.sources[slot], fresh.get(slot), fall);
    if (fresh.has(slot) || v[0] > SILENT || v[1] > SILENT) next[slot] = v;
  }
  meters.sources = next;
  fetchMeters();
}

/** Démarre la boucle ; renvoie la fonction d'arrêt. */
export function startMeters() {
  last = 0;
  raf = requestAnimationFrame(frame);
  return () => cancelAnimationFrame(raf);
}
