# kettler-run7

Projet pour piloter un tapis de course **Kettler RUN 7** en Bluetooth Low Energy, l'application officielle Kettler n'existant plus.

**État : première version de l'application en cours** (Tauri 2 + Rust + Svelte 5, même architecture qu'OpenGitClient). Le protocole a été vérifié sur le vrai tapis le 2026-11-03, depuis un Mac (Python + [bleak](https://github.com/hbldh/bleak)). Les scripts de ces tests sont dans [research/](research/), leurs journaux dans [research/logs/](research/logs/).

## Application

### Commandes

```bash
npm install                # une fois (.npmrc force legacy-peer-deps : npm 10 plante sinon sur les peers de vitest)
npm run app:dev            # lance l'appli (Vite :1430 puis Rust ; 1420 est pris par OpenGitClient) — À LANCER DANS LE TERMINAL INTÉGRÉ (Bluetooth)
npm run app:sim            # même appli, tapis SIMULÉ (KETTLER_SIM=1) : aucun Bluetooth, marche aussi depuis Bash
npm run check              # svelte-check (vite build ne vérifie pas les types)
npm test                   # vitest
npm run build              # frontend seul → dist/
cd src-tauri && cargo test # tests Rust (protocole, logique d'état) — sans Bluetooth, OK depuis Bash
```

### macOS 12 (Monterey) minimum

L'application doit tourner sur **macOS 12**. Ce qui en découle :
- `tauri.conf.json` → `bundle.macOS.minimumSystemVersion = "12.0"` (Tauri en tire `LSMinimumSystemVersion` et `MACOSX_DEPLOYMENT_TARGET`) et `signingIdentity = "-"` (signature ad hoc de tout le bundle, identifiant = celui de l'appli, pour que l'autorisation Bluetooth soit rattachée à l'appli).
- Beaucoup de Mac sous macOS 12 sont des **Intel** : pour les distribuer, `npm run app:build:universal` (arm64 + x86_64, cible Rust `x86_64-apple-darwin` installée avec `rustup target add`). Résultat dans `src-tauri/target/universal-apple-darwin/release/bundle/`. Non signée par un développeur Apple : au premier lancement, clic droit → Ouvrir.
- **La WebView est le WebKit du système**, donc au pire celui de Safari 15.0 (macOS 12.0 ; Safari 16–17 seulement si le Mac a été mis à jour). `vite.config.ts` cible `safari15`. Ne pas utiliser sans repli : container queries (`cqi`/`cqh`, `@container` : Safari 16), `color-mix()` (16.2), `:has()`, `dvh`, `@layer`, `:focus-visible` (15.4), `light-dark()`, nesting CSS ; côté JS : `.at()`, `structuredClone`, `Object.hasOwn`, `findLast` (15.4), `toSorted` (16), `Promise.withResolvers` (17.4), `requestIdleCallback` (jamais).
- Les tailles des chiffres dépendent des container queries : chaque usage a un bloc `@supports not (container-type: size)` qui estime la colonne des mesures avec `--kr-col-w` / `--kr-col-h` (définies dans `src/app.css`). En ajouter un à toute nouvelle déclaration en `cq*`. Le halo vert utilise le token `--kr-go-halo` au lieu de `color-mix()`.
- Rust : rien de spécifique (btleplug/CoreBluetooth, Tauri 2 : macOS 10.15+).

### Simulateur (le tapis est prêté : plus de tests en direct)

`npm run app:sim` lance l'appli avec `KETTLER_SIM=1` : le Rust pilote alors `treadmill/sim.rs` au lieu de btleplug (aucun Bluetooth ouvert, titre de fenêtre « Kettler RUN 7 — SIMULATEUR »). Le simulateur reçoit **les mêmes trames** que le vrai tapis et répond avec les mêmes octets : toute la session (handshake, polling, `WRITE_QUIET`, reconnexion, nouvel essai de démarrage) tourne comme avec le RUN 7. Il reproduit ce qui est documenté plus bas (« Comportements observés ») : handshake obligatoire, réponses par paquets de ~480 ms en notifications de 20 octets, ~7 requêtes/s, écriture collée perdue, compte à rebours, rampes, à-coups de distance, second arrêt = remise à zéro, démarrage ignoré après une longue pause, bande qui continue pendant une coupure, clé retirée = arrêt net + module coupé.

Le menu orange **Simulateur** de l'en-tête affiche la console simulée et provoque les événements (`SimAction` dans `dto.rs`) : boutons de la console (démarrer, stop, ±0,5 km/h, ±1 %), mains sur les capteurs, coupure radio 10/30 s, retrait/remise de la clé, console muette 8 s, prochaine commande en échec, réponse au CRC invalide, pause longue. Le journal `[sim]` du terminal montre chaque écriture reçue (ou perdue).

**Limite** : le simulateur encode ce qu'on SAIT du RUN 7 ; ce qui n'a jamais été mesuré (pas de la console, consignes envoyées bande arrêtée, Online pendant le compte à rebours…) y est une hypothèse. Toute nouvelle fonctionnalité qui touche au protocole reste à valider sur le vrai tapis quand il reviendra.

Les tests `cargo test` jouent des séances complètes de `Treadmill` contre le simulateur (`treadmill/session.rs`, module `tests`), en temps tokio mis en pause (`start_paused`) : plusieurs minutes simulées en quelques millisecondes.

Hors Tauri (`npx vite` dans un navigateur), l'interface utilise à la place le tapis TypeScript `src/lib/mock.ts` (sans le Rust).

`cargo test` / `cargo check` passent depuis l'outil Bash ; tout ce qui ouvre réellement le Bluetooth (l'appli lancée) doit tourner dans le terminal intégré de Claude Desktop. L'appli empaquetée a sa propre autorisation grâce à `src-tauri/Info.plist` (`NSBluetoothAlwaysUsageDescription`).

**Commentaires de code en français ; interface en français.**

**Interface : desktop uniquement, design « Piste »** (fenêtre minimale 900×680). Sources du design dans [docs/design/](docs/design/) : `piste-tokens.dc.html` (direction, tokens) et `piste-console.dc.html` (console, tous les états). Tous les styles passent par les tokens `--kr-*` de `src/app.css` ; une couleur vive signifie toujours un état (vert démarrer, ambre pause, rouge stop et alertes, orange avertissement, bleu donnée). Polices Barlow et Barlow Semi Condensed embarquées via `@fontsource` : pas de Google Fonts, l'appli marche hors ligne. Non repris du design : courbe de vitesse et objectif (options facultatives).

### Architecture

```
src-tauri/src/
  main.rs              → kettler_run7_lib::run()
  lib.rs               Builder Tauri, état géré, enregistrement des commandes
  dto.rs               CONTRAT : types envoyés au frontend (TreadmillSnapshot…), EVENT_STATE
  kettler/protocol.rs  protocole pur : CRC, trames, handshake, FrameDecoder (aucune I/O)
  treadmill/session.rs session (btleplug ou simulateur) : scan, connexion, handshake, polling, écritures, reconnexion
  treadmill/state.rs   logique d'état pure (Model)
  treadmill/sim.rs     tapis simulé au niveau du protocole (KETTLER_SIM=1)
  treadmill/profile.rs profils de course : profils de départ, validation, stockage (profiles.json), déroulement (ProfileRun)
  commands.rs          commandes Tauri (fines : délèguent à treadmill/)
src/
  main.ts, App.svelte, app.css
  lib/types.ts         miroir TypeScript de dto.rs
  lib/api.ts           invoke/listen ; bascule sur un tapis simulé (lib/mock.ts) hors Tauri
  lib/sim.svelte.ts    mode simulateur (navigateur ou Rust) et état du tapis simulé
  lib/profiles.ts      profils : bornes et validation (miroir de profile.rs), résumé, ProfileRunner (tapis simulé du navigateur)
  lib/profiles.svelte.ts liste des profils, écran affiché (console / éditeur), profil en cours d'édition
  lib/components/      écran de pilotage, éditeur de profils (ProfileEditor, ProfileChart, StepField), ProfilePanel, ProfileStrip
```

**Le Rust possède tout l'état et toute la sécurité** ; le frontend affiche les `TreadmillSnapshot` reçus et appelle des commandes. Il ne parle jamais Bluetooth.

### Contrat frontend ↔ Rust

Événement `treadmill-state` (payload `TreadmillSnapshot`, voir `src-tauri/src/dto.rs`), émis à chaque changement d'état.

Commandes (toutes `async`, erreur = `String` lisible en français) :

| Commande | Arguments | Effet |
|---|---|---|
| `get_state` | — | Renvoie le `TreadmillSnapshot` courant |
| `connect` | — | Scan (≤ 20 s) du tapis dont le nom sans espaces vaut `run7` (insensible à la casse), connexion, abonnement `FFF1`, handshake, lecture des valeurs statiques (nom, id, article, bornes), puis polling |
| `disconnect` | — | Déconnexion voulue : pas de reconnexion automatique |
| `start` | — | Écrit Online = 0 (compte à rebours 3-2-1 puis 0,8 km/h) |
| `stop` | — | Écrit Online = 1. **Prioritaire** : part immédiatement, avant toute écriture en attente, et est toujours accepté |
| `set_target_speed` | `kmh: f32` | Arrondi à 0,1 ; borné à `[speed_min, min(speed_cap, speed_max)]` ; refusé si la bande ne tourne pas |
| `set_target_incline` | `pct: f32` | Arrondi à 0,1 ; borné aux limites du tapis ; refusé si la bande ne tourne pas |
| `set_speed_cap` | `kmh: f32` | Plafond appliqué à toute consigne (1–20 km/h, défaut **6,0**). S'il descend sous la consigne courante, la consigne est ramenée au plafond |
| `list_profiles` | — | Profils de départ (lecture seule) puis profils de l'utilisateur, par nom |
| `save_profile` | `profile: RunProfile` | Valide et enregistre (crée si `id` est vide) ; rend le profil enregistré. Refusé pour un profil de départ, et pour le profil en cours bande en marche |
| `delete_profile` | `id: String` | Supprime un profil de l'utilisateur (même refus) |
| `select_profile` | `id: Option<String>` | Profil déroulé pendant la séance (gardé dans `profiles.json`). Un profil ne se choisit que bande arrêtée ; `null` (aucun) toujours accepté |
| `sim_status` | — | `SimStatus` du tapis simulé, ou `null` avec le vrai tapis (sert aussi à détecter le mode simulateur) |
| `sim_action` | `action: SimAction` | Événement sur le tapis simulé (`{ kind: "linkLoss", seconds }`…) ; refusé avec le vrai tapis |

Règles de la session :
- Polling : **au plus 3 lectures en attente de réponse** (`MAX_INFLIGHT`), 60 ms entre deux envois ; sans réponse en 700 ms, on repart. Chaque cycle (≥ 500 ms) lit Online, SpeedSet, SpeedGet, InclineSet, InclineGet ; un cycle sur deux ajoute Distance, Energy, Pulse. Après une écriture, la valeur écrite est relue en priorité. ⚠️ Deux limites constatées sur le RUN 7 : la console ne traite que ~6 à 8 requêtes/s (à 16 lectures/s, sa file se remplissait : commandes en 2 à 5 s, voire perdues), et le module BLE ne renvoie les réponses que **par paquets, toutes les ~480 ms** (avec une seule lecture en attente, l'écran ne se rafraîchissait que toutes les ~2 s). Délai mesuré d'une commande (envoi → consigne relue) : ~1,4 s. Les écritures passent entre deux lectures, avec 250 ms de silence avant et après. Écritures BLE en morceaux de 20 octets, sans réponse.
- `elapsed_s` : s'accumule tant que `running == Some(true)` ; remis à 0 quand la distance repart à une valeur inférieure à la précédente (compteur remis à zéro par le tapis).
- Perte de liaison non voulue → `connection = Reconnecting`, `lost_while_running = (running == Some(true))`, nouvelle tentative toutes les 2 s jusqu'au succès ou à un `disconnect`. Au succès, `lost_while_running` repasse à faux.
- Toute réponse avec un CRC invalide est ignorée (on attend le polling suivant).

### Profils de course

Un profil = une suite de segments (distance en m, vitesse, inclinaison). Stockés dans `profiles.json` (dossier de données de l'appli : `~/Library/Application Support/<identifiant>/`), avec le profil choisi ; un fichier illisible est renommé `.json.illisible`, jamais écrasé. Trois profils de départ en dur (`starter_profiles`, toutes vitesses ≤ 6,0 km/h = plafond par défaut), en lecture seule : on les duplique pour les adapter. Bornes d'un segment : 50–10 000 m, 0,8–20 km/h, 0–15 % ; 60 segments au plus. Le TS (`profiles.ts`) recopie bornes et messages pour l'éditeur ; le Rust revalide.

Déroulement (choix de l'utilisateur, 2026-11-09), entièrement côté Rust (`ProfileRun` + `Model::profile_order`, appelé en fin de cycle de polling par `Treadmill::drive_profile`) :
- Choisir un profil ne lance rien. Après « Démarrer », le 1er segment part à la fin du compte à rebours (consigne relue ≥ vitesse min).
- La progression est la **somme des avancées** du compteur Distance, pas sa valeur : une remise à zéro pendant `Treadmill::start` (démarrage ignoré après une longue pause) ne fait pas reculer le profil. Une remise à zéro hors démarrage, ou un STOP envoyé bande déjà arrêtée (fin de séance), le ramène au début. Pause : progression gardée, consignes du segment renvoyées à la reprise.
- Changement de segment : vitesse et inclinaison envoyées une fois (bornées et plafonnées comme toute consigne, seulement ce qui diffère), puis vérifiées à la relecture ; non relues en 5 s → renvoi, 3 envois au plus. Ensuite le profil ne touche plus à rien jusqu'au segment suivant : **un réglage manuel (appli ou console) vaut jusqu'à la fin du segment** (`manual` dans `ProfileProgress`).
- Après le dernier segment : récupération lente, min(3,0 km/h, vitesse du dernier segment) et 0 %, **sans arrêt automatique**.
- Les envois du profil passent par `write_frame` avec la garde `stop_gen` : un STOP les annule comme toute commande.
- Interface : section « Profil » de la colonne de droite (liste, ✎ voir/modifier, + créer ; bande en marche : nom + « Sans profil ») ; bande `ProfileStrip` sous la vitesse (segment, mètres restants, aperçu, segment suivant annoncé en orange dans les derniers mètres : min(60 m, 40 % du segment) ; vitesse affichée = vitesse plafonnée). **L'éditeur n'est accessible que bande arrêtée** (`canEditProfiles`) ; si la bande démarre, App.svelte revient d'office à la console (brouillon gardé, « reprendre » dans la section Profil).
- Clavier dans l'éditeur : Échap envoie toujours l'arrêt ; **Espace dans un champ texte tape une espace** (seule exception à « Espace = arrêt », sûre car l'éditeur n'existe que bande arrêtée). En pause, Échap y termine donc la séance, comme sur la console.
- À valider sur le vrai tapis : enchaînement des consignes vitesse + inclinaison au changement de segment (deux écritures de suite, chacune avec `WRITE_QUIET`).

## Git

### Branches : git flow

| Branche | Rôle |
|---|---|
| `main` | Versions publiées uniquement. Chaque merge dans `main` est tagué `vX.Y.Z`. On n'y commite jamais directement |
| `develop` | Branche d'intégration, point de départ de tout nouveau travail |
| `feature/<sujet>` | Nouvelle fonctionnalité, partie de `develop`, fusionnée dans `develop` |
| `bugfix/<sujet>` | Correction d'un bug pas encore publié, partie de `develop`, fusionnée dans `develop` |
| `release/X.Y.Z` | Préparation d'une version (numéros de version, derniers correctifs), partie de `develop`, fusionnée dans `main` (tag `vX.Y.Z`) puis dans `develop` |
| `hotfix/X.Y.Z` | Correction urgente d'une version publiée, partie de `main`, fusionnée dans `main` (tag) puis dans `develop` |

- Noms de branches en minuscules, mots séparés par des tirets (`feature/plafond-vitesse`).
- Merges en `--no-ff` pour garder la trace de chaque branche.
- La configuration git flow est déjà dans `.git/config` (`gitflow.*`) : l'outil `git flow` (`brew install git-flow-avh`) peut s'en servir tel quel, mais il n'est pas obligatoire.
- Numéro de version (SemVer) à changer en même temps dans `package.json`, `src-tauri/Cargo.toml` et `src-tauri/tauri.conf.json`, sur la branche `release/` ou `hotfix/`.

### Messages de commit : Conventional Commits

```
<type>(<portée>): <description à l'impératif, en minuscule, sans point final>

<corps facultatif : le pourquoi>

<pied facultatif : BREAKING CHANGE: …, Refs: …>
```

- **Types** : `feat` (fonctionnalité), `fix` (correction), `docs`, `style` (mise en forme sans effet), `refactor`, `perf`, `test`, `build` (dépendances, Cargo, npm, bundle), `ci`, `chore`, `revert`.
- **Portées** usuelles : `protocol` (`kettler/protocol.rs`), `session` (`treadmill/`), `commands`, `dto`, `ui` (frontend), `research` (scripts et journaux), `design`, `deps`.
- Changement incompatible : `!` après le type (`feat(dto)!: …`) et pied `BREAKING CHANGE: …`.
- Messages rédigés **en français**, comme le code et l'interface.
- Exemples : `feat(session): arrêter la bande à la reconnexion si elle tournait seule`, `fix(protocol): ne pas échapper le CRC des trames envoyées`, `docs: noter l'unité d'Energy confirmée sur la console`.

## Sécurité — à lire avant tout envoi de commande

- Le tapis est une machine qui peut blesser. **Ne jamais envoyer d'écriture (démarrage, vitesse, inclinaison) sans l'accord explicite de l'utilisateur et sans quelqu'un à côté du tapis**, clé de sécurité en place.
- Les lectures sont sans danger ; tout le reste demande l'accord.
- Pendant un démarrage à distance, l'utilisateur doit rester sur les rebords latéraux, pas sur la bande.
- Les scripts de test plafonnent leurs consignes (3,0 km/h, 3,0 %) et terminent toujours par un arrêt (dans un `finally`). Garder ce principe dans l'application : arrêt toujours accessible (bouton PAUSE/STOP dès qu'une séance est en cours ou que l'état est incertain, et Échap/Espace qui envoient TOUJOURS l'arrêt, quel que soit l'affichage), plafond de vitesse configurable, arrêt en cas de perte de connexion si possible.
- Le bouton stop de la console et la clé de sécurité restent prioritaires sur le Bluetooth.

## Environnement macOS (important)

- **Le Bluetooth ne marche pas depuis l'outil Bash de Claude Code** : macOS (TCC) tue le processus (exit 134, SIGABRT), avec ou sans sandbox.
- **Lancer les scripts Bluetooth dans le terminal intégré de Claude Desktop** (outil `run_in_terminal`), qui a l'autorisation Bluetooth :
  `uv run --with bleak python research/run7_kettler.py sortie.txt`
  puis lire le fichier de sortie (ou `read_terminal`).
- **Ne pas utiliser l'app Terminal de macOS** (`open -a Terminal …`) : l'utilisateur l'a demandé explicitement.
- Python système = 3.9 ; `uv` est disponible (`/opt/homebrew/bin/uv`), bleak s'installe à la volée avec `uv run --with bleak`.

## Identité de l'appareil

| Champ | Valeur |
|---|---|
| Nom BLE annoncé | `RUN 7` (**avec un espace** — comparer en retirant les espaces) |
| Adresse vue par macOS | `BC3138B2-5201-FDDD-7608-C855DE0BC676` (UUID propre à ce Mac, pas une adresse MAC) |
| UUID de service annoncé | `00001101-0000-1000-8000-00805f9b34fb` |
| Module Bluetooth | Microchip/ISSC, modèle `BT5050`, matériel `5050_SPP`, firmware `2030030` |
| DeviceType (protocole) | `T` (treadmill) |
| DeviceName (protocole) | `RUN 7` |
| DeviceId (protocole) | `STB1T0158` |
| Référence d'article (valeur n° 22) | `07883-600` = référence Kettler du RUN 7 ([notice de montage](https://manualshive.com/en/manual/kettler/07883-600/1959525?page=6)) |

Pas de service FTMS (`0x1826`) : les applis compatibles FTMS (Zwift, Kinomap…) ne le pilotent pas directement.

## Services GATT

| Service | Caractéristique | Propriétés | Rôle |
|---|---|---|---|
| `0000180a` Device Information | 2A29, 2A24, 2A25, 2A27, 2A26, 2A28, 2A23, 2A2A | read | Infos du module (fabricant « ISSC », modèle « BT5050 »…) |
| `49535343-fe7d-4ae5-8fa9-9fafd205e455` (ISSC) | `49535343-6daa-4d02-abf6-19569aca69fe` | read, write | Paramètres de connexion du module (lu : `00 18 00 18 00 00 00 48 00`). Non utilisé. |
| | `49535343-aca3-481c-91ec-d85e28a60318` | notify, write | Non utilisé, jamais rien reçu. |
| **`00001101-0000-1000-8000-00805f9b34fb`** | **`0000fff2-0000-1000-8000-00805f9b34fb`** | write-without-response | **Envoi des trames vers le tapis** |
| | **`0000fff1-0000-1000-8000-00805f9b34fb`** | notify | **Réponses du tapis** |

Le module est un pont série (UART) : `FFF2`/`FFF1` transportent le protocole Kettler. Les écritures sont découpées en **morceaux de 20 octets** (prudence MTU ; ça fonctionne). Une réponse peut arriver en plusieurs notifications, ou plusieurs réponses dans une seule : il faut un parseur à flux d'octets.

## Protocole Kettler

Source : la bibliothèque Rust **kdri** v0.4.3 ([docs.rs/crate/kdri](https://docs.rs/crate/kdri/latest/source/src/lib.rs)), écrite pour les appareils Kettler en Bluetooth classique (RFCOMM). Le même protocole passe tel quel sur le pont BLE `FFF1`/`FFF2`. kdri reconnaît les noms commençant par `TOUR`, `RACER`, `ERGO`, `RECUMBENT`, `UNIX`, `SKYLON`, `RUN`, `TRACK`.

### Trame

```
0x02 | payload échappé | 0x03 | CRC_hi CRC_lo (échappés aussi)
```

- **Échappement** (byte stuffing) : tout octet `0x02`, `0x03` ou `0x10` est remplacé par `0x10, octet ^ 0x20`. Ça s'applique au payload **et aux 2 octets de CRC** (piège rencontré : une réponse à 1,9 km/h a un CRC `03 73`, transmis `10 23 73`).
- **CRC16** sur le payload *non échappé* : polynôme réfléchi `0x8408` (CCITT/Kermit), valeur initiale `0x0000`, pas de XOR final, **envoyé poids fort en premier**.

```python
TABLE = []
for i in range(256):
    crc, d = 0, i
    for _ in range(8):
        crc = (crc >> 1) ^ 0x8408 if (d ^ crc) & 1 else crc >> 1
        d >>= 1
    TABLE.append(crc)

def crc16(bs):
    c = 0
    for b in bs:
        c = (c >> 8) ^ TABLE[(c ^ b) & 0xff]
    return c
```

### Payload

| Octets | Champ |
|---|---|
| 2 | Identifiant de valeur (u16 big-endian) |
| 1 | Instruction : `1` = lecture, `2` = écriture, `3` = réponse |
| 2 | Longueur des données `n` (u16 big-endian) |
| n | Données (u16 big-endian pour les nombres, ASCII pour les textes) |

Une lecture a `n = 0`. Le tapis répond à chaque lecture **et à chaque écriture** par une instruction `3` contenant la valeur.

### Message d'identification (handshake) — obligatoire

**Sans ce message dans les secondes qui suivent la connexion, le tapis reste muet et coupe la liaison au bout d'environ 10 s.** Payload (23 octets, valeur n° 1, 18 octets de données), tiré de kdri. ⚠️ Son octet d'instruction vaut `01` (lecture), pas `02` : c'est une « lecture » qui porte des données. Ne pas la reconnaître sur l'instruction :

```
00 01 01 00 12 2d 24 f2 24 96 a4 ff 98 29 f9 21 be 9d aa 9e 4d 01 17
```

Trame complète envoyée (vérifiée identique à la référence kdri) :

```
02 00 01 01 00 12 2d 24 f2 24 96 a4 ff 98 29 f9 21 be 9d aa 9e 4d 01 17 03 86 77
```

Le tapis répond par une trame « valeur n° 1, instruction 3 » qui renvoie les mêmes 18 octets. Ensuite il répond à tout et garde la connexion tant qu'on l'interroge (sessions de 48 s testées sans coupure).

### Exemples de trames vérifiées

| Commande | Trame |
|---|---|
| Lire SpeedGet (35) | `02 00 23 01 00 00 03 f0 42` |
| Écrire SpeedSet = 25 (2,5 km/h) | `02 00 22 10 22 00 10 22 00 19 03 a3 46` |
| Écrire InclineSet = 10 (1,0 %) | `02 00 1e 10 22 00 10 22 00 0a 03 77 78` |
| Démarrer (Online = 0) | `02 00 26 10 22 00 01 00 03 7b a7` |
| Arrêter (Online = 1) | `02 00 26 10 22 00 01 01 03 6a 2e` |
| Réponse DeviceType | `02 00 18 10 23 00 01 54 03 c6 d4` → `'T'` |

## Valeurs (identifiants)

Unités : vitesse en **0,1 km/h**, inclinaison en **0,1 %**.

| N° | Nom | Type | Accès | Vérifié sur le RUN 7 |
|---|---|---|---|---|
| 1 | (handshake) | 18 octets | écriture | Valeur du message d'identification. Lue, elle répond avec 0 octet |
| 2 | *inconnu* | u16 | lecture | `0x013A` = 314. Hors kdri. Probablement une version (protocole ou firmware 3.14 ?) |
| 6 | DeviceState | u16 | lecture | Toujours `1`, en marche, à l'arrêt et jusqu'au retrait de la clé (kdri : 0 = Up, 1 = Down ; sens réel inconnu). Inutile en pratique |
| 8 | Pulse | u16 | lecture | ✅ Pouls des capteurs de poignées en bpm (`89` lu en tenant les poignées), `0` sans contact |
| 15 | Distance | u16 | lecture | ✅ **Mètres** : 182 lus pour 0,18 km affichés sur la console, et ~178 m recalculés à partir de SpeedGet. Plus précis que la console. Avance par à-coups (peut rester figé quelques secondes puis bondir de ~40 m) : lisser l'affichage |
| 16 | Energy | u16 | lecture | Augmente avec l'effort. **Unité probable : 0,1 kcal** (105 après ~3 min 40 à 3 km/h ; 10,5 kcal est plausible, 105 kcal non). Pas confirmée sur la console |
| 17 | Time | u16 | lecture | **Ne répond pas.** La console affiche le temps en `minutes.secondes` (`3.40` = 3 min 40 s, cohérent avec la durée entre les passages d'Online à 0 puis à 1) : le calculer côté application |
| 18 | TimeMode | u16 | lecture | **Ne répond pas** |
| 20 | DeviceName | ASCII | lecture | `RUN 7` |
| 21 | *inconnu* | 4 octets | lecture | `06 ac fc 01` = 112 000 001 en u32 big-endian. Hors kdri. Nombre trop « rond » pour un compteur : sans doute un code de version (`112.000.001` ?) |
| 22 | *référence article* | ASCII | lecture | `07883-600` : la référence Kettler du RUN 7 (vérifiée) |
| 23 | *inconnu* | ASCII | lecture | `001` : probablement une variante ou une révision matérielle |
| 24 | DeviceType | ASCII 1 car. | lecture | `T` |
| 25 | DeviceId | ASCII | lecture | `STB1T0158` |
| 30 | InclineSet | u16 | lecture/écriture | ✅ consigne d'inclinaison |
| 31 | InclineGet | u16 | lecture | ✅ inclinaison réelle |
| 32 | InclineMin | u16 | lecture | `0` |
| 33 | InclineMax | u16 | lecture | `150` (15,0 %) |
| 34 | SpeedSet | u16 | lecture/écriture | ✅ consigne de vitesse |
| 35 | SpeedGet | u16 | lecture | ✅ vitesse réelle de la bande |
| 36 | SpeedMin | u16 | lecture | `8` (0,8 km/h) |
| 37 | SpeedMax | u16 | lecture | `200` (20,0 km/h) |
| 38 | Online | u8 | lecture/écriture | ✅ marche/arrêt, voir ci-dessous |

Valeurs propres aux vélos chez kdri, pas pertinentes ici : 7 BrakeMode, 9 Rpm, 10–13 Power*, 26 InPowerRange, 27–29 BrakeLevel*. Le n° 14 (Speed) est obsolète.

**Scan complet en lecture (2026-11-03, bande arrêtée, [research/run7_scan_ids.py](research/run7_scan_ids.py))** : seuls 1, 2, 6, 8, 15, 16, 20–25 et 30–38 répondent. Tout le reste de 0 à 299 reste muet, en particulier 0, 3–5, 7, 9–14, **17–19**, 26–29 et 39 et au-delà. Aucune valeur cachée ne donne le temps écoulé.

### Marche / arrêt (Online, n° 38)

- **Valeur brute inversée** par rapport au booléen « online » de kdri : **`0` = en marche, `1` = arrêté**.
- **Écrire `0` = démarrer** : compte à rebours 3-2-1 sur la console (~5 s), puis la bande part à la vitesse minimale (0,8 km/h). Pendant le compte à rebours, les autres écritures sont ignorées.
- **Écrire `1` = arrêter / mettre en pause** : la bande décélère (2,0 → 0 km/h en ~5 s) et SpeedSet repasse à 0. Distance et calories sont gardées (pause). **Un second arrêt, bande déjà arrêtée, les remet à zéro** (vérifié le 2026-11-03 : distance 100 → 0).
- ⚠️ **Démarrage ignoré après une longue pause** (vérifié le 2026-11-03) : resté longtemps en pause avec ses compteurs, le tapis ignore Online = 0. Il l'accepte après un second arrêt (remise à zéro). Juste après un arrêt, en revanche, Online = 0 relance bien la bande. L'appli essaie donc Online = 0, et si la bande n'est pas partie après 7 s, envoie Online = 1, attend 1 s, puis renvoie Online = 0 (`Treadmill::start` dans `treadmill/session.rs`). Le seuil de « longue pause » est inconnu (kdri parle de 5 min).
- **Pause / fin de séance** (confirmé par l'utilisateur, comme la console) : un premier STOP met la séance en pause, compteurs gardés ; un second STOP la termine et remet distance et calories à zéro. Interface (`stopButtonMode` dans `src/lib/controls.ts`, demandé par l'utilisateur, desktop uniquement) : bande en marche → bouton **PAUSE** (ambre) ; en pause → **STOP** « Terminer la séance » (rouge) à côté de « Reprendre » ; arrêtée sans séance ou non connectée → aucun bouton ; pendant une reconnexion → bouton toujours affiché. Après un PAUSE accepté, le bouton passe aussitôt à STOP sans attendre la relecture (~1,4 s). Les deux envoient le même Online = 1. Conséquence assumée : Échap/Espace en pause terminent la séance.
- Online passe aussi à `0`/`1` quand on démarre ou arrête depuis la console : c'est un bon indicateur d'état. Un arrêt à la console passe Online à 1 et SpeedSet à 0 immédiatement, puis la bande décélère de 3,0 à 0 km/h en ~8 s.

### Consigne et vitesse réelle

- **SpeedSet (34) est la consigne**, ce que l'utilisateur ou l'application demande (ex. `30` = 3,0 km/h réglé à la console).
- **SpeedGet (35) est la vitesse réelle mesurée** de la bande. Elle oscille autour de la consigne (29–30 pour une consigne de 30) et la rejoint progressivement après chaque changement (rampe).
- Même logique pour l'inclinaison : InclineSet (30) = consigne, InclineGet (31) = position réelle.
- L'application doit afficher la vitesse réelle et piloter la consigne.

## Comportements observés

- **Vitesse** : une écriture de SpeedSet est acceptée directement, bande en marche, sans écrire Online d'abord. La bande suit avec une rampe d'environ 0,1 km/h par demi-seconde (2,0 → 2,5 km/h en ~4 s).
- **Inclinaison** : une écriture d'InclineSet est acceptée en ~2 s ; le moteur atteint 1,0 % en ~3 s, et redescend à 0 en ~3 s.
- ⚠️ **Perte de la connexion BLE : la bande continue de tourner.** Testé le 2026-11-03 : 25 s de coupure à 3 km/h, la bande a continué sans changer de vitesse, Online toujours à 0 à la reconnexion. L'application ne peut donc pas compter sur la coupure pour arrêter le tapis. Elle doit afficher clairement la perte de connexion et se reconnecter (handshake compris) au plus vite.
- **Retrait de la clé de sécurité** : la bande s'arrête, la console affiche un message d'erreur, et la **liaison BLE est coupée** (le module semble perdre l'alimentation ou être réinitialisé). DeviceState ne change pas avant la coupure. L'application doit traiter une déconnexion soudaine comme un arrêt possible par la clé.
- Les réglages faits sur la console (boutons +/−) se voient immédiatement dans SpeedSet/InclineSet : l'application doit relire les consignes plutôt que supposer les siennes.
- Sans handshake, aucune requête ne reçoit de réponse : FitShow, Domyos, iConsole, Sole, WalkingPad et texte Kettler (`ST\r\n`…) ont tous été essayés sans succès.
- Polling utilisé dans les tests : 4 à 11 lectures enchaînées toutes les ~80 ms, puis une pause de 0,6 à 0,8 s. Le tapis tient ce rythme sans erreur (186/186 réponses avec CRC valide).
- ⚠️ **Une écriture collée à une lecture est perdue** (vérifié le 2026-11-03 dans l'appli) : la console traite une requête à la fois, et ne répond jamais aux écritures, même acceptées. L'appli impose 250 ms de silence avant ET après chaque écriture (`WRITE_QUIET`). Les scripts Python marchaient parce qu'ils écrivaient toujours après ≥ 0,6 s de silence.

## Questions ouvertes

- **CRC échappé dans les trames ENVOYÉES** : le tapis échappe le CRC de ses réponses, et `protocol.rs` fait donc de même à l'envoi. Mais nos scripts Python (et kdri) ne l'échappaient pas, et toutes les écritures vérifiées avaient un CRC sans octet spécial. Seules ces commandes sont concernées : SpeedSet **64 et 66** (6,4 / 6,6 km/h) et InclineSet **13, 28, 46, 68, 70** (1,3 / 2,8 / 4,6 / 6,8 / 7,0 %). Aucune lecture, ni Online. À vérifier sur le tapis : si une de ces consignes est ignorée, ne pas échapper le CRC à l'envoi.
- Unité d'Energy (16) : 0,1 kcal très probable, à confirmer en relevant les calories de la console à l'arrêt.
- Temps écoulé : Time (17) ne répond pas et aucune autre valeur ne le fournit (scan 0–299) → à calculer côté application à partir des passages d'Online.
- Signification exacte des n° 2 (314), 21 (112000001) et 23 (`001`).
- Sens de DeviceState (6), toujours à 1, même au retrait de la clé.
- Pouls via une ceinture cardio (seuls les capteurs des poignées ont été testés).
- Choix de la techno de l'application (pas encore décidé). Piste : appli web en **Web Bluetooth** dans Chrome (Mac, Android, Windows ; pas Safari/iOS), qui donne accès à `FFF1`/`FFF2` sans les contraintes TCC du terminal. Non testé.

## Fichiers

- [research/run7_kettler.py](research/run7_kettler.py) — référence du protocole : trame, CRC, parseur à flux, handshake, **lectures seulement**. Le point de départ de toute implémentation.
- [research/run7_speedtest.py](research/run7_speedtest.py) — test d'écriture de vitesse (+0,5 km/h, plafond 3,0 km/h, puis retour), seulement si la bande tourne déjà.
- [research/run7_inclinetest.py](research/run7_inclinetest.py) — test d'écriture d'inclinaison (+1,0 %, plafond 3,0 %, puis retour).
- [research/run7_startstop.py](research/run7_startstop.py) — test arrêt → démarrage → arrêt via Online, termine toujours à l'arrêt.

- [research/run7_observe.py](research/run7_observe.py) — test guidé par annonces vocales (`say`), **lecture seule** : 3 min d'enregistrement pendant que l'utilisateur marche, coupure BLE de 25 s bande en marche, reconnexion, arrêt et retrait de la clé à la console. Le retrait de la clé coupe la liaison, ce qui fait finir le script sur une erreur `BleakError` (attendu).
- [research/run7_scan_ids.py](research/run7_scan_ids.py) — lit chaque numéro de valeur d'une plage (0–63 par défaut, ou `<début> <fin>` en arguments 2 et 3). **Lecture seule.**

Tous prennent en argument un fichier de sortie : `uv run --with bleak python research/<script>.py sortie.txt`.
