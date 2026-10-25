# Paint

Éditeur d'images matricielles inspiré de Paint pour Windows 11, écrit en **Rust**.
Le moteur de dessin est compilé en **WebAssembly**. L'application de bureau
multiplateforme utilise **Tauri**. Linux est la cible prioritaire (.deb, .rpm, AppImage).

## Architecture

```
crates/paint-core   Moteur en Rust pur : calques, outils, pinceaux, formes, sélection,
                    historique (annuler/rétablir), presse-papiers, codecs (PNG, JPEG, BMP,
                    GIF, TIFF, WEBP, ICO). Aucune dépendance au navigateur, entièrement testé.
crates/paint-wasm   Liaisons wasm-bindgen : le moteur compilé en WebAssembly (ui/pkg).
ui/                 Interface (HTML/CSS/JS) qui transmet les événements au moteur et
                    affiche l'image directement depuis la mémoire WebAssembly.
src-tauri/          Application de bureau en Rust (Tauri 2) : fenêtre, presse-papiers
                    système (GTK sous Linux, arboard sous Windows/macOS), fichiers,
                    boîtes de dialogue, ouverture d'un fichier passé en ligne de commande.
```

Toute la logique (outils, sélection, poignées, historique) se trouve dans le moteur Rust.
L'interface se contente de lui transmettre les événements de pointeur et les commandes.

## Fonctionnalités

- **Sélection** : rectangle et forme libre ; déplacement, copie (Ctrl+glisser), redimensionnement
  avec 8 poignées (Maj = proportions), flèches du clavier, sélectionner tout, inverser, supprimer,
  sélection transparente, rogner, menu contextuel (clic droit).
- **Presse-papiers système** : couper/copier/coller des images entre Paint et les autres
  applications ; le collage d'une image plus grande que le canevas l'agrandit ; « Coller depuis un fichier ».
- **Outils** : crayon, remplissage (tolérance), texte (police, taille, gras, italique, souligné,
  barré, fond opaque), gomme (clic droit = remplacement de couleur), sélecteur de couleur, loupe.
- **Pinceaux** : pinceau, calligraphie 1 et 2, aérographe, huile, pastel, marqueur, crayon naturel, aquarelle.
- **Formes (23)** : ligne, courbe (2 inflexions), ovale, rectangles, polygone, triangles, losange,
  pentagone, hexagone, 4 flèches, 3 étoiles, 3 bulles, cœur, éclair ; contour/remplissage ;
  forme modifiable (déplacer, redimensionner, recolorer) jusqu'à validation.
- **Image** : rogner, redimensionner et incliner, rotations, retournements, suppression
  de l'arrière-plan, propriétés (taille du canevas), poignées de redimensionnement du canevas sur les 8 côtés.
- **Calques** : ajouter, dupliquer, fusionner vers le bas, supprimer, masquer, réordonner
  (boutons ou glisser), renommer, miniatures.
- **Couleurs** : couleur 1 et 2, palette de 20 couleurs + 10 personnalisées, éditeur TSV/RVB/hex.
- **Affichage** : zoom de 10 % à 800 % (molette + Ctrl, curseur, préréglages, ajuster à la
  fenêtre), règles, quadrillage, barre d'état, plein écran, thème clair/sombre, français/anglais.
- **Fichiers** : ouvrir, enregistrer, enregistrer sous (7 formats), imprimer, glisser-déposer,
  confirmation des modifications non enregistrées (y compris à la fermeture).

## Lancer Paint

```bash
./run --web        # version web : compile le moteur WASM, sert l'appli et ouvre le navigateur
./run --linux      # application de bureau Linux (ajouter --release pour la version optimisée)
```

Autres modes : `./run --package` (paquets .deb/.rpm/AppImage), `./run --web-build [dossier]`
(site statique à héberger), `./run --test` (toutes les suites de tests), `./run --help`.
`./run --web --host 0.0.0.0 --port 8080` rend la version web accessible sur le réseau local.

La version web est une application installable (PWA) : bouton « Installer » du navigateur.
Une fois chargée, elle fonctionne hors ligne. Elle s'ouvre aussi avec « Ouvrir avec » pour les images.

## Installation sous Linux

Après la compilation (voir ci-dessous), les paquets sont dans `target/release/bundle/` :

```bash
sudo apt install ./target/release/bundle/deb/Paint_1.0.0_amd64.deb
```

ou, sans installation :

```bash
./target/release/bundle/appimage/Paint_1.0.0_amd64.AppImage
```

Le paquet .deb déclare Paint comme éditeur des fichiers PNG, JPEG, BMP, GIF, TIFF, WEBP et ICO
(menu « Ouvrir avec »).

## Compilation

Prérequis : Rust (cible `wasm32-unknown-unknown`), `wasm-pack`, Node.js.
Les en-têtes WebKitGTK sont pris dans le système s'ils sont installés
(`sudo apt install libwebkit2gtk-4.1-dev`). Sinon, `scripts/linux-devlibs.sh` les télécharge
sans droits administrateur.

```bash
scripts/build-linux.sh        # application + paquets .deb, .rpm, AppImage
npm run dev                   # lance l'application de bureau en développement
npm run build:wasm && npm run serve   # version navigateur sur http://127.0.0.1:4173
```

Sous Windows et macOS, `npm run build` produit les installateurs natifs (MSI/NSIS, DMG).

## Tests

| Suite | Commande | Contenu |
|---|---|---|
| Moteur (Rust) | `cargo test -p paint-core` | 183 tests : chaque outil, chaque pinceau et chaque forme, toute la sélection, le presse-papiers, l'historique, les calques, les formats, et 26 tests de non-régression issus des audits |
| Moteur en WebAssembly | `npm run test:wasm` | l'API compilée en WASM, exécutée dans Node |
| Interface (navigateur) | `npm run build:wasm && npm run test:e2e` | 99 tests Playwright qui pilotent l'interface réelle : boutons, menus, raccourcis (clavier AZERTY compris), navigation au clavier, fenêtres étroites, tactile, sélection à la souris, presse-papiers système, fichiers, calques, mode hors ligne |
| Bureau (natif) | `cargo test -p paint` | fichiers (enregistrements simultanés, permissions, liens), ligne de commande, presse-papiers GTK contre un vrai serveur X (copier vers une autre application et coller depuis une autre application) |
| Application de bureau | `scripts/test-desktop.sh` | 8 tests WebDriver sur l'application Tauri réelle (WebKitGTK) : souris, clavier, presse-papiers natif, enregistrement et ouverture, format d'après l'extension, confirmation à la fermeture |

`./run --test` lance toutes ces suites. `cargo run --release -p paint-core --example bench` mesure les
opérations interactives sur une image de 12 mégapixels.

## Limites

- Taille maximale d'une image : 16 384 pixels par côté et 64 mégapixels. Les opérations qui
  dépasseraient ces limites sont refusées avec un message, au lieu de saturer la mémoire.
- Version web : les navigateurs réservent Ctrl+N et Ctrl+W ; utilisez les menus
  Fichier › Nouveau et Image › Redimensionner.
- La zone de texte ne se déplace pas et ne fait pas de retour à la ligne automatique (Entrée
  pour aller à la ligne).

Les tests du presse-papiers GTK et de l'application de bureau ont besoin d'un affichage :
`DISPLAY` est utilisé s'il est défini, sinon `scripts/test-desktop.sh` démarre Xvfb.

## Raccourcis

| Action | Raccourci |
|---|---|
| Nouveau / Ouvrir / Enregistrer | Ctrl+N / Ctrl+O / Ctrl+S |
| Enregistrer sous (PNG) | F12, Ctrl+Maj+S |
| Imprimer | Ctrl+P |
| Propriétés de l'image | Ctrl+E |
| Redimensionner et incliner | Ctrl+W |
| Annuler / Rétablir | Ctrl+Z / Ctrl+Y (ou Ctrl+Maj+Z) |
| Couper / Copier / Coller | Ctrl+X / Ctrl+C / Ctrl+V |
| Sélectionner tout | Ctrl+A |
| Supprimer la sélection | Suppr |
| Rogner | Ctrl+Maj+X |
| Déplacer la sélection | Flèches (Maj = 10 px) |
| Valider / annuler (forme, sélection) | Entrée / Échap |
| Zoom avant / arrière | Ctrl++ / Ctrl+-, Ctrl+molette |
| 100 % / Ajuster à la fenêtre | Ctrl+1 / Ctrl+0 |
| Règles / Quadrillage | Ctrl+R / Ctrl+G |
| Nouveau calque | Ctrl+Maj+N |
| Plein écran | F11 |
| Texte : gras / italique / souligné, valider | Ctrl+B / Ctrl+I / Ctrl+U, Ctrl+Entrée |
