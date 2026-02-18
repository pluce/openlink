# openlink-gui

Interface graphique DCDU / ATC pour le réseau OpenLink.

Construite avec [Dioxus](https://dioxuslabs.com/) 0.7 en mode desktop.

## Fonctionnalités

- **Multi-onglets** : Ouvrez simultanément des stations ATC et des cockpits DCDU.
- **DCDU (avion)** : Interface pilote pour le logon, la connexion et la réception de messages CPDLC.
- **ATC** : Interface contrôleur avec gestion des stations connectées, acceptation/rejet de logon, et envoi de demandes de connexion.
- **Historique** : Les stations précédemment ouvertes sont mémorisées pour un accès rapide.

## Lancement

```bash
cd crates/openlink-gui
cargo run
```

## Architecture

```
src/
  main.rs          — Point d'entrée Dioxus desktop
  state.rs         — État global (onglets, stations mémorisées)
  nats_client.rs   — Gestion de la connexion NATS / authentification
  components/
    tab_bar.rs     — Barre d'onglets
    station_setup.rs — Formulaire de configuration d'une station
    dcdu_view.rs   — Interface DCDU (pilote)
    atc_view.rs    — Interface ATC (contrôleur)
    shared.rs      — Composants partagés (badges, listes de messages)
```
