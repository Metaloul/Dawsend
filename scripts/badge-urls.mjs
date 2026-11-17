// Construit les liens qui pré-remplissent le formulaire du badge OpenSSF Best
// Practices (niveau passing) avec les réponses de docs/BADGE-OPENSSF.md.
//
//   node scripts/badge-urls.mjs
//
// Trois liens (pour rester sous la longueur maximale d'une URL). Le site ne remplit
// que les champs encore vides, et rien n'est enregistré avant « Save » : chaque
// proposition se relit dans le formulaire. Les critères « à confirmer » restent à
// remplir à la main. Doc : github.com/ossf/best-practices-badge, docs/automation-proposals.md
import { readFileSync } from "node:fs";
const doc = "docs/BADGE-OPENSSF.md";
const R = "https://github.com/Metaloul/Dawsend";
const PROJECT = 16849;
const base = `https://www.bestpractices.dev/en/projects/${PROJECT}/passing/edit?`;
const groups = []; let cur = null;
for (const line of readFileSync(doc, "utf8").split(/\r?\n/)) {
  if (line.startsWith("## ")) { cur = { title: line.slice(3), rows: [] }; groups.push(cur); continue; }
  const m = line.match(/^\| ([a-z0-9_]+) \| ([^|]+) \| (.+) \|$/);
  if (!m || !cur) continue;
  const [, id, status, just] = m;
  if (status.includes("confirmer")) continue; // à la main
  const j = just.replace(/`R(\/[^`]*)?`/g, (_, p) => R + (p || "")).replace(/`/g, "");
  cur.rows.push([id, status.trim(), j]);
}
const enc = (s) => encodeURIComponent(s).replace(/%20/g, "+");
const parts = [["Basics", "Change control"], ["Reporting", "Quality"], ["Security", "Analysis"]];
const urls = parts.map((names, i) => {
  const q = [];
  if (i === 0) q.push("name=Dawsend", "description=" + enc("Sends a DAW's ASIO audio (e.g. Studio One) to a regular Windows output so that screen sharing (Discord) can hear it: a VST3/CLAP plugin plus a tray app."), "license=" + enc("MIT AND GPL-3.0-only"), "implementation_languages=" + enc("Rust, JavaScript, Svelte"));
  let n = 0;
  for (const g of groups.filter((g) => names.includes(g.title))) for (const [id, st, j] of g.rows) { q.push(`${id}_status=${enc(st)}`, `${id}_justification=${enc(j)}`); n++; }
  return { names, n, url: base + q.join("&") };
});
for (const u of urls) console.log(`
${u.names.join(" + ")} (${u.n} critères)
${u.url}`);
