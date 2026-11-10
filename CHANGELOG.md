# Journal des versions

Toutes les évolutions notables de l'application. Format inspiré de
[Keep a Changelog](https://keepachangelog.com/fr/1.1.0/), numéros de version
[SemVer](https://semver.org/lang/fr/). Chaque version publiée est un merge dans
`main` tagué `vX.Y.Z` (voir CLAUDE.md, « Git »).

## [Non publié]

### Ajouté
- Bip d'avertissement avant chaque changement de segment d'un profil : deux bips
  courts puis un aigu, au moment où le segment suivant s'annonce en orange.
- Son de changement de segment (« ding-dong » montant) quand le profil passe
  effectivement au segment suivant, ou à la récupération après le dernier.
- Import d'un parcours GPX (« Importer un parcours GPX », section Profil) : le
  relief devient un nouveau profil, ouvert dans l'éditeur avant tout
  enregistrement. Pente mesurée tous les 100 m sur l'altitude lissée, descentes
  à 0 %, segments de 200 m au moins (60 au plus). Les vitesses du fichier ne sont
  pas reprises : une même vitesse pour tous les segments, réglable d'un coup.
- Carte du parcours (bouton « Carte » de l'éditeur, profils importés d'un GPX) :
  tracé découpé selon les segments (un clic choisit le segment), coloré au
  choix selon l'inclinaison ou la vitesse (5 teintes de bleu, légende), départ et
  arrivée. La vitesse et l'inclinaison du segment choisi se règlent directement depuis la carte,
  dans un encart en bas à droite (‹ › pour passer d'un segment à l'autre,
  « Zoomer sur la section ») ; un bouton sous le zoom revient à tout le tracé. Fonds Plan IGN, photos aériennes IGN ou OpenStreetMap. Le tracé
  (allégé à 1000 points au plus) est enregistré avec le profil ; les profils
  importés avant cette version n'en ont pas : réimporter le fichier.
- Dénivelé du segment choisi dans l'encart de la carte, pour information : sa
  montée sur le tapis (distance × inclinaison), en mètres.

### Corrigé
- Le message d'erreur noir ne recouvre plus les mesures du bas (inclinaison,
  calories, pouls) pendant toute une reconnexion ou toute une séance : pendant
  une reconnexion, la cause (tapis introuvable…) s'affiche dans le bandeau
  orange ; les autres erreurs disparaissent d'elles-mêmes au bout de 10 s.

## [0.2.0] — 2026-11-09

### Ajouté
- Profils de course : suites de segments (distance, vitesse, inclinaison) déroulées
  automatiquement pendant la séance. Choix dans la colonne de droite, éditeur avec
  aperçu graphique (bande arrêtée seulement), progression sous la vitesse avec le
  segment suivant annoncé. Trois profils de départ : Marche douce, Collines, Fractionné.
- Numéro de version affiché dans l'en-tête ; `npm run version:set -- X.Y.Z` l'écrit
  dans tous les fichiers, `npm run version:check` (et `npm test`, `cargo test`)
  vérifie qu'ils concordent.
- Releases GitHub automatiques : chaque tag `vX.Y.Z` publie l'application macOS
  universelle (`.dmg`, `.app` zippée, sommes SHA-256) avec les notes de ce journal.

### Retiré
- Plafond de vitesse (commande `set_speed_cap`, réglage « Plafond ») : les consignes
  ne sont plus bornées qu'aux limites annoncées par le tapis (0,8–20 km/h).

## [0.1.0] — 2026-11-09

### Ajouté
- Première version : connexion Bluetooth au RUN 7 (handshake Kettler), affichage des
  mesures, démarrage, pause / fin de séance, vitesse et inclinaison, plafond de vitesse,
  reconnexion automatique, arrêt par Échap / Espace.
- Tapis simulé au niveau du protocole (`npm run app:sim`).
