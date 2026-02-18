# Architecture OpenLink - Implémentation de Référence

Ce repository contient l'implémentation de référence du système OpenLink (ACARS Next Gen).
Le cœur est écrit en Rust, avec une définition stricte des protocoles via JSON Schema et une architecture orientée messages (MOM) sur NATS.

## Structure du Workspace (Mono-repo)

| Composant | Path | Description |
|-----------|------|-------------|
| **Schemas** | [`schemas/`](./schemas) | **Source de Vérité**. Contrats d'interface (JSON Schema) agnostiques. |
| **Models** | [`crates/openlink-models`](./crates/openlink-models) | Bibliothèque de types Rust générée automatiquement. |
| **SDK** | [`crates/openlink-sdk`](./crates/openlink-sdk) | Kit de développement client (Authentification, Chiffrement, Transport NATS). |
| **Session Manager** | [`crates/openlink-cpdlc`](./crates/openlink-cpdlc) | **Routeur Intelligent**. Résout les indicatifs (ex: "LFPG") en adresses réseau (CID). |
| **Auth Service** | [`crates/openlink-auth`](./crates/openlink-auth) | Service de Token (STS). Échange les tokens OIDC contre des JWT NATS signés. |
| **CLI** | [`crates/openlink-cli`](./crates/openlink-cli) | Client de démonstration (Pilot/ATC) et outil de test. |
| **Mock IDP** | [`crates/mock-oidc`](./crates/mock-oidc) | Simulateur de fournisseur d'identité (OAuth2/OIDC) pour le dev local. |

## Démarrage Rapide

### Pré-requis
- Rust (Cargo)
- Docker (pour NATS)

### 1. Lancer l'infrastructure
```bash
docker-compose up -d
```

### 2. Démarrer les Services Core
Dans des terminaux séparés :

**Mock Identity Provider :**
```bash
# Génère les identités à la volée sur http://localhost:4000
cargo run -p mock-oidc
```

**Service d'Authentification :**
```bash
# Valide les tokens OIDC et émet les JWT NATS
cargo run -p openlink-auth
```

**Session Manager (Routeur) :**
```bash
# Gère le routage logique des messages CPDLC
cargo run -p openlink-cpdlc
```

### 3. Utiliser le CLI (Démo Inter-Processus)

Le CLI permet d'envoyer des messages atomiques pour tester le flux.

#### Étape A : Lancer un ATC en écoute (ex: LFPG)
Dans un terminal :
```bash
cargo run -p openlink-cli -- \
  --network-id vatsim --network-address ATC \
  acars --callsign LFPG --address LFPGCYA \
  cpdlc --aircraft-callsign AFR123 --aircraft-address AY213 --atc \
  listen
```

#### Étape B : Envoyer un Logon Request (Pilote -> LFPG)
Dans un autre terminal :
```bash
cargo run -p openlink-cli -- \
  --network-id vatsim --network-address PILOT \
  acars --callsign AFR123 --address AY213 \
  cpdlc --aircraft-callsign AFR123 --aircraft-address AY213 --pilot \
  send logon-request --station LFPG --origin LFPG --destination EGLL
```

#### Étape C : Répondre au Logon (LFPG -> Pilote)
Dans un troisième terminal (ou après avoir arrêté l'écoute de l'étape A) :
```bash
cargo run -p openlink-cli -- \
  --network-id vatsim --network-address ATC \
  acars --callsign LFPG --address LFPGCYA \
  cpdlc --aircraft-callsign AFR123 --aircraft-address AY213 --atc \
  send logon-response --accepted
```

## Scénarios Supportés

### 1. Connexion Initiale (Logon)
1. Le pilote envoie un `LOGON REQUEST` à LFPG.
2. L'ATC (LFPG) accepte le Logon (`LOGON RESPONSE`).
3. L'ATC initie la connexion (`CONNECTION REQUEST`).
4. Le pilote confirme (`CONNECTION CONFIRM`).

### 2. Transfert Connecté (Figure 2-17 ICAO)
1. L'ATC A (LFPG) initie un transfert vers l'ATC B (EGLL).
2. Séquence Automatique :
   - LFPG envoie `NDA Notification` (Next Data Authority) au pilote.
   - LFPG envoie `Transfer Request` à EGLL.
   - EGLL accepte et envoie `Transfer Response`.
   - EGLL envoie `Connection Request` au pilote (via le routage Session Manager).
   - Le pilote confirme la connexion avec EGLL.
   - EGLL notifie LFPG de la fin du transfert.
   - LFPG termine la session (`Terminated`).

## Fonctionnalités Clés

### Routage Dynamique
Les pilotes s'adressent aux stations par leur nom (ex: "LFPG"). Le Session Manager intercepte ces demandes, résout les adresses techniques (ex: "LFPGCYA") via le fichier de configuration et route le message via NATS.

### Sécurité Zero Trust
- **Authentification Forte** : Le CLI s'authentifie d'abord auprès de `mock-oidc` pour obtenir un ID Token, puis l'échange contre un JWT NATS via `openlink-auth`.
- **Jetons Signés** : Les permissions NATS sont limitées par utilisateur (CID).

### Typage Fort
Tout message circulant sur le réseau est garanti conforme aux schémas JSON grâce à la librairie `openlink-models`.
