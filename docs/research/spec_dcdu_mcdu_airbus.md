# Spécifications Techniques : DCDU & MCDU CPDLC (Airbus A320 Family)

## 1. Introduction
Ce document détaille les spécifications fonctionnelles, visuelles et cinématiques du système DCDU (Datalink Control and Display Unit) et de son intégration avec le MCDU pour les communications CPDLC (Controller-Pilot Data Link Communications).

## 2. Description du Hardware (DCDU)
Le DCDU est composé de deux unités identiques installées au-dessus des écrans ECAM.
- **Écran :** LCD couleur (Physical resolution typique 800x480, mais logique de grille 24x7).
- **Boutons :** 
  - `BRT/DIM` : Contrôle de luminosité.
  - `LSK (Line Selection Keys)` : 3 de chaque côté (1L-3L, 1R-3R).
  - `ATC MSG` : Bouton rétroéclairé Ambre clignotant/fixe sur le GlareShield.

## 3. Interface Utilisateur (DCDU Design System)

### 3.1 Structure du Layout (Grille)
- **Dimensions :** 24 caractères x 7 lignes.
- **Header (Ligne 1) :** Titre de la page ou État du système (Cyan). Centré.
- **Body (Lignes 2 à 5) :** Texte du message ATC (Blanc pour Uplink, Magenta pour Preview).
- **Status (Ligne 6) :** Feedback d'action (ex: `STBY SENT`, `SENDING`, `SENT`). Cyan.
- **Actions (Ligne 7) :** Prompts LSK (Vert/Ambre/Cyan).

### 3.2 Detailed Design System Specifications

#### 3.2.1 Typography & Character Anatomy (Airbus Digital)
- **Police :** `Airbus Digital` (Monospace semi-condensé).
- **Formatage :** Tout en majuscules (Full Caps).
- **Métriques logiques précises (Base 432x252 px) :**
  - **Grille :** 24 colonnes x 7 lignes.
  - **Cellule de caractère (Cell Size) :** 18px (L) x 36px (H).
  - **Taille Glyphe :** 14px x 24px (centré dans la cellule).
  - **Interligne (Leading) :** 36px (Baseline à Baseline).
  - **Inter-caractère (Tracking) :** Fixe (Monospace).
  - **Épaisseur de trait (Stroke) :** 2.0px constant.
- **Anatomie des Caractères :**
  - **Slashed Zero :** Le `0` possède une barre diagonale de bas-gauche à haut-droite (angle de 45°).
  - **Chiffre 1 :** Possède une base horizontale (serif) et un crochet supérieur (flag).
  - **Chiffre 7 :** Possède une barre horizontale médiane (style européen).
  - **Lettre Q :** La queue du `Q` est un trait diagonal droit descendant sous la baseline (descender).
  - **Lettre M & W :** Segments droits, jonctions pointues sur la baseline/cap-height.
  - **Symbol Set :**
    - `*` (Astérisque) : 6 branches, rayon 6px.
    - `[` `]` (Brackets) : Hauteur totale de la cellule moins 4px.
    - `>` `<` (Arrows) : Chevrons 90°, épaisseur 2px.

#### 3.2.2 Palette de Couleurs (Physical Simulation)
Les couleurs simulent un écran AMLCD avec un léger "glow" (bloom 2px).
| Token | Hex | RGB | Luminance | Usage |
| :--- | :--- | :--- | :--- | :--- |
| `COLOR_BG` | `#020202` | 2, 2, 2 | 0.5% | Fond (noir profond non-parfait). |
| `WHITE_UPLINK` | `#EBEDEF` | 235, 237, 239 | 92% | Messages reçus (Uplink). |
| `MAGENTA_PREV` | `#F032E6` | 240, 50, 230 | 70% | Prévisualisation/Downlink (Preview). |
| `CYAN_SYS` | `#00E1E1` | 0, 225, 225 | 80% | Titres, brackets, labels, info système. |
| `GREEN_POS` | `#32CD32` | 50, 205, 50 | 85% | WILCO, ROGER, SEND, ACCEPT. |
| `AMBER_NEG` | `#FF9100` | 255, 145, 0 | 85% | UNABLE, REJECT, ERROR, CANCEL. |

#### 3.2.3 Transitions & Comportement d'Écran
- **Global Blanking :** Transition de **80ms à 100ms** (écran `#000000`) lors de chaque changement de page ou mise à jour de contenu (simule le refresh de l'unité d'affichage).
- **Video Inverse (Select) :** Inversion des couleurs (Fond = Couleur du texte, Texte = Noir) pendant **600ms** lors de l'activation d'un LSK.
- **Blinking (Notification) :** 1Hz (500ms ON / 500ms OFF). Cycle synchrone avec le GlareShield.

### 4. Cinématographie des Interactions

#### 4.1 Composition de Message (MCDU -> DCDU Preview)

##### 4.1.1 Flux de Composition Simple
1. **État Initial :** DCDU affiche `ATC IDLE` ou le dernier message reçu.
2. **Action MCDU :** Le pilote saisit `350` dans le scratchpad et l'insère dans `[ALT]` sur la page `ATC MSG CONSTRUCTION`.
3. **Action MCDU :** Appui sur `PREVIEW*` (LSK 6R).
4. **Transition DCDU :**
   - **T+0ms :** L'écran passe au noir total.
   - **T+80ms :** Affichage du layout `PREVIEW`.
   - **Ligne 1 :** `ATC PREVIEW` (Cyan).
   - **Ligne 2 :** `CLIMB TO FL350` (Magenta).
   - **Ligne 7 :** `*SEND` (Vert, 3R).
5. **Validation :** L'appui sur `*SEND` sur le DCDU déclenche l'envoi. Le DCDU affiche `SENDING` (Cyan) en L6.

##### 4.1.2 Flux de Composition Compound (Multi-élements)
- **Règle de Concaténation :** Chaque nouvel élément ajouté sur le MCDU entraîne un rafraîchissement complet du DCDU (Blanking).
- **Visualisation :**
  - Élément 1 : `CLIMB TO FL350` (L2).
  - Élément 2 : `PROCEED DIRECT TO ABC` (L4).
- **Gestion du Débordement :** 
  - Si le message dépasse 4 lignes de texte, le DCDU génère automatiquement une pagination.
  - Ligne 7 affiche `NEXT PAGE >` (Cyan, 3R) au lieu de `*SEND`.
  - Le bouton `*SEND` n'apparaît que sur la **dernière page** du message de preview.

#### 4.2 Logique de Réponse sur DCDU (Uplink Flow)

##### 4.2.1 Cycle de Réponse Positive/Négative
- **Réception :** `ATC MSG` clignote. Appui sur le bouton GlareShield.
- **Affichage :** Message Uplink en Blanc.
- **Prompts :** `*WILCO` (1R, Vert) et `*UNABLE` (3R, Ambre).
- **Interaction :**
  - Clic sur `*WILCO`.
  - **Feedback :** `*WILCO` passe en Video Inverse (Fond vert, texte noir) pendant 600ms.
  - **Transmission :** Le DCDU affiche `SENDING...` en L6.
  - **Confirmation :** `SENT` apparaît en L6. Le bouton GlareShield s'éteint.

##### 4.2.2 Logique STANDBY (DM1)
- **Utilité :** Informer le contrôleur que la demande est prise en compte mais nécessite du temps.
- **Cinématique :**
  - Appui sur `*STBY` (2R, Cyan).
  - Le prompt `*STBY` clignote une fois en Video Inverse et disparaît définitivement.
  - Status L6 affiche `STBY SENT` pendant 3.0s.
  - **Persistance :** Les options `*WILCO` et `*UNABLE` restent affichées. L'Uplink reste considéré comme "Ouvert".

##### 4.2.3 Boucle UNABLE avec Raison
1. **Action :** Appui sur `*UNABLE`.
2. **Interlock MCDU :** Le DCDU affiche `SELECT REASON ON MCDU` (Ambre clignotant, L6).
3. **MCDU :** La page `ATC REASON` s'affiche automatiquement (ou le prompt `REASON` apparaît en 6L).
4. **Cinématique de Retour :** Une fois la raison sélectionnée (ex: `DUE TO WEATHER`), le DCDU se met à jour :
   - Texte passe en Magenta : `UNABLE DUE TO WEATHER`.
   - Bouton `*SEND` (Vert) apparaît en L7.

### 4.4 Gestion de la Queue (Message Queueing)
Le DCDU peut gérer plusieurs messages en attente.
1. **Indicateur de Queue :** Si plus d'un message est reçu, le titre (L1) affiche `MSG 1/2` (Cyan) à droite.
2. **Navigation :**
   - Appui sur `ATC MSG` (GlareShield) : Affiche le message suivant dans la queue.
   - Si un message est ouvert et qu'un nouveau arrive : Le DCDU ne change pas automatiquement, mais le bouton GlareShield se remet à clignoter.
3. **Priorité :** Les messages sont empilés par ordre chronologique d'arrivée. Les messages de type "Urgent" sont placés en tête de pile.

### 4.5 Notification GlareShield (Comportement Précis)
- **Réception :** Clignotement Ambre (1Hz).
- **Consultation :** Passe au fixe lors de l'affichage du message.
- **Réponse envoyée :** S'éteint si aucune autre notification n'est en attente.

---

## 5. Synchronisation et Dualité
- **Sync :** Les réponses sont synchronisées entre Capt et F/O.
- **Indépendance :** La navigation (pages, queue) est locale à chaque unité.
- **Conflit :** Premier arrivé, premier servi par l'ATSU.
