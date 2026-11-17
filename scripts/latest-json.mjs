// Écrit le manifeste de mise à jour (latest.json) lu par l'appli, et les notes de la
// release tirées de CHANGELOG.md.
//
//   node scripts/latest-json.mjs <version> <installateur.exe> <installateur.exe.sig> [dépôt]
//
// Sorties, à côté de l'installateur : latest.json et release-notes.md.
import { readFileSync, writeFileSync } from "node:fs";
import { basename, dirname, join } from "node:path";

const [version, installer, sigPath, repo = "Metaloul/Dawsend"] = process.argv.slice(2);
if (!version || !installer || !sigPath) {
  console.error("usage : node scripts/latest-json.mjs <version> <installateur.exe> <signature.sig> [dépôt]");
  process.exit(2);
}

// Section « ## X.Y.Z … » de CHANGELOG.md, jusqu'au titre suivant.
const changelog = readFileSync("CHANGELOG.md", "utf8").replace(/\r\n/g, "\n");
const start = changelog.search(new RegExp(`^## ${version.replace(/\./g, "\.")}\b.*$`, "m"));
if (start < 0) {
  console.error(`CHANGELOG.md n'a pas de section « ## ${version} » : pas de release sans changelog.`);
  process.exit(1);
}
const rest = changelog.slice(start).split("\n").slice(1);
const end = rest.findIndex((l) => l.startsWith("## "));
const notes = (end < 0 ? rest : rest.slice(0, end)).join("\n").trim();

const url = `https://github.com/${repo}/releases/download/v${version}/${basename(installer)}`;
const entry = { signature: readFileSync(sigPath, "utf8").trim(), url };
const manifest = {
  version,
  notes,
  pub_date: new Date().toISOString(),
  // Le plugin de mise à jour cherche « windows-x86_64-nsis » puis « windows-x86_64 ».
  platforms: { "windows-x86_64": entry, "windows-x86_64-nsis": entry },
};

const dir = dirname(installer);
writeFileSync(join(dir, "latest.json"), JSON.stringify(manifest, null, 2) + "\n");
writeFileSync(join(dir, "release-notes.md"), notes + "\n");
console.log(`latest.json et release-notes.md écrits dans ${dir} (version ${version})`);
