#!/usr/bin/env node
// Notes d'une release GitHub : la section `## [X.Y.Z]` de CHANGELOG.md, suivie
// des consignes d'installation. Échoue si la version n'a pas de section (on ne
// publie pas une version sans journal).
//
//   node scripts/release-notes.mjs 0.2.0 > notes.md

import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const version = (process.argv[2] ?? "").replace(/^v/, "");

if (!/^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/.test(version)) {
  console.error("Usage : node scripts/release-notes.mjs X.Y.Z");
  process.exit(1);
}

const changelog = readFileSync(join(root, "CHANGELOG.md"), "utf8");
const lines = changelog.split("\n");
const escaped = version.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
const start = lines.findIndex((l) => new RegExp(`^## \\[${escaped}\\]`).test(l));
if (start < 0) {
  console.error(`CHANGELOG.md n'a pas de section « ## [${version}] » : ajoutez-la avant de publier.`);
  process.exit(1);
}
let end = lines.findIndex((l, i) => i > start && /^## /.test(l));
if (end < 0) end = lines.length;
const body = lines.slice(start + 1, end).join("\n").trim();
if (!body) {
  console.error(`La section « ## [${version}] » de CHANGELOG.md est vide.`);
  process.exit(1);
}

console.log(`${body}

---

### Installation (macOS 12 ou plus récent, Intel et Apple Silicon)

1. Télécharger le \`.dmg\` ci-dessous, l'ouvrir et glisser **Kettler RUN 7** dans Applications.
2. L'application n'est pas signée par un développeur Apple : au premier lancement,
   **clic droit → Ouvrir**, puis confirmer.
3. Autoriser le Bluetooth quand macOS le demande.

⚠️ Le tapis est une machine qui peut blesser : clé de sécurité en place et quelqu'un
à côté du tapis pour tout démarrage à distance.`);
