#!/usr/bin/env node
// Numéro de version de l'application (SemVer), tenu à jour dans 5 fichiers :
// package.json, package-lock.json, src-tauri/Cargo.toml, src-tauri/Cargo.lock
// et src-tauri/tauri.conf.json.
//
//   node scripts/version.mjs            → affiche la version, vérifie la cohérence
//   node scripts/version.mjs 0.3.0      → écrit 0.3.0 partout (branche release/ ou hotfix/)
//
// La version vient de package.json : l'interface l'affiche (vite.config.ts) ;
// le Rust la lit dans Cargo.toml, et ses tests vérifient qu'elle est identique.

import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const SEMVER = /^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/;
const CRATE = "kettler-run7";

const read = (f) => readFileSync(join(root, f), "utf8");
const write = (f, text) => writeFileSync(join(root, f), text);

/** Chaque fichier : comment lire sa version, comment la remplacer. */
const FILES = [
  {
    file: "package.json",
    get: (t) => JSON.parse(t).version,
    set: (t, v) => t.replace(/^(\s*"version":\s*")[^"]*(")/m, `$1${v}$2`),
  },
  {
    file: "package-lock.json",
    get: (t) => {
      const lock = JSON.parse(t);
      const a = lock.version;
      const b = lock.packages?.[""]?.version;
      return a === b ? a : `${a} / ${b}`;
    },
    // Les deux premières occurrences : la racine et `packages[""]`.
    set: (t, v) => {
      let n = 0;
      return t.replace(/^(\s*"version":\s*")[^"]*(")/gm, (m, a, b) => (n++ < 2 ? `${a}${v}${b}` : m));
    },
  },
  {
    file: "src-tauri/Cargo.toml",
    get: (t) => /^\[package\][\s\S]*?^version\s*=\s*"([^"]*)"/m.exec(t)?.[1],
    set: (t, v) => t.replace(/^(\[package\][\s\S]*?^version\s*=\s*")[^"]*(")/m, `$1${v}$2`),
  },
  {
    file: "src-tauri/Cargo.lock",
    get: (t) => new RegExp(`^name = "${CRATE}"\\nversion = "([^"]*)"`, "m").exec(t)?.[1],
    set: (t, v) => t.replace(new RegExp(`^(name = "${CRATE}"\\nversion = ")[^"]*(")`, "m"), `$1${v}$2`),
  },
  {
    file: "src-tauri/tauri.conf.json",
    get: (t) => JSON.parse(t).version,
    set: (t, v) => t.replace(/^(\s*"version":\s*")[^"]*(")/m, `$1${v}$2`),
  },
];

function versions() {
  return FILES.map(({ file, get }) => ({ file, version: get(read(file)) ?? "(introuvable)" }));
}

const wanted = process.argv[2];

if (wanted) {
  if (!SEMVER.test(wanted)) {
    console.error(`Version invalide : « ${wanted} » (attendu : X.Y.Z, SemVer).`);
    process.exit(1);
  }
  for (const { file, set } of FILES) write(file, set(read(file), wanted));
}

const found = versions();
const distinct = [...new Set(found.map((f) => f.version))];
for (const { file, version } of found) console.log(`${version.padEnd(12)} ${file}`);
if (distinct.length !== 1) {
  console.error("\nVersions différentes : lancez `npm run version:set -- X.Y.Z`.");
  process.exit(1);
}
console.log(wanted ? `\nVersion ${wanted} écrite partout.` : `\nVersion ${distinct[0]} : cohérente.`);
