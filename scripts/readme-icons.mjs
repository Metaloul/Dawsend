#!/usr/bin/env node
// Génère les icônes du README (docs/icons/) à partir de Primer Octicons (MIT), le jeu
// d'icônes de GitHub. Une couleur fixe, lisible sur les thèmes clair et sombre de
// GitHub (une image <img> ne peut pas suivre la couleur du texte).
//
// Usage :
//   npm pack @primer/octicons@19.42.0 && tar -xzf primer-octicons-19.42.0.tgz
//   node scripts/readme-icons.mjs ./package
//
// Pour ajouter une icône : l'ajouter à ICONS (nom Octicons sans « -16.svg »), relancer.
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const pkg = process.argv[2];
if (!pkg) {
  console.error("usage : node scripts/readme-icons.mjs <dossier du paquet @primer/octicons>");
  process.exit(1);
}

// Braise (assets/palette.md) : 3,1:1 sur fond blanc, 5,4:1 sur le fond sombre de GitHub.
const ACCENT = "#E0601F";

// Fichier de sortie → [icône Octicons, couleur]
const ICONS = {
  stack: ["stack", ACCENT],
  clock: ["clock", ACCENT],
  pulse: ["pulse", ACCENT],
  zap: ["zap", ACCENT],
  "device-desktop": ["device-desktop", ACCENT],
  plug: ["plug", ACCENT],
  "shield-check": ["shield-check", ACCENT],
  sync: ["sync", ACCENT],
  globe: ["globe", ACCENT],
  log: ["log", ACCENT],
  lock: ["lock", ACCENT],
  verified: ["verified", ACCENT],
  search: ["search", ACCENT],
  hash: ["hash", ACCENT],
  download: ["download", ACCENT],
  book: ["book", ACCENT],
  bug: ["bug", ACCENT],
  "file-badge": ["file-badge", ACCENT],
  pin: ["pin", ACCENT],
  // Voyants du plugin, aux couleurs de l'interface (assets/palette.md).
  "led-ok": ["dot-fill", "#3DD68C"],
  "led-warn": ["dot-fill", "#F2B83B"],
  "led-bad": ["dot-fill", "#F25555"],
  "led-dim": ["dot-fill", "#6B7280"],
};

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const out = join(root, "docs", "icons");
mkdirSync(out, { recursive: true });

for (const [file, [name, color]] of Object.entries(ICONS)) {
  const src = readFileSync(join(pkg, "build", "svg", `${name}-16.svg`), "utf8");
  if (!src.startsWith("<svg ")) throw new Error(`${name} : SVG inattendu`);
  const svg = src.replace("<svg ", `<svg fill="${color}" `);
  writeFileSync(join(out, `${file}.svg`), svg + "\n");
}
writeFileSync(join(out, "LICENSE-octicons.txt"), readFileSync(join(pkg, "LICENSE"), "utf8"));
const version = JSON.parse(readFileSync(join(pkg, "package.json"), "utf8")).version;
console.log(`${Object.keys(ICONS).length} icônes écrites dans docs/icons (Octicons ${version})`);
