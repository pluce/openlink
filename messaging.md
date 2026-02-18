Voici une spécification fonctionnelle et technique détaillée concernant les règles générales du protocole CPDLC, la gestion des états des messages (chaînage, MIN/MRN), et les attributs de réponse. Ce document est structuré pour permettre à un développeur d'implémenter la machine à états de votre système.

### 1. Structure de Base des Messages

La communication CPDLC repose sur l'échange de messages formatés, divisés en deux catégories :

* **UM (Uplink Message) :** Messages émis par le système sol (ATC) vers l'avion `[1]`.
* **DM (Downlink Message) :** Messages émis par l'avion vers le système sol `[1]`.

Un message CPDLC peut être constitué d'un seul élément de message (Single-element) ou d'une combinaison pouvant aller jusqu'à 5 éléments de message (Multi-element message) `[1]`.

### 2. Le Mécanisme de Chaînage : MIN et MRN

Le protocole CPDLC étant asynchrone, il utilise un système de pointeurs pour lier une réponse à une requête ou une instruction spécifique. C'est le cœur de la machine à états.

* **MIN (Message Identification Number) :** Pour chaque connexion CPDLC, l'avion et le système au sol attribuent un MIN à chaque nouveau message qu'ils génèrent. Il s'agit d'un nombre entier strictement compris entre 0 et 63 inclus `[1]`. L'avion et le sol gèrent leur propre compteur MIN de manière totalement indépendante (le sol incrémente les MIN pour les UM, l'avion incrémente les MIN pour les DM) `[1]`.
* **MRN (Message Reference Number) :** Lorsqu'un message est généré en *réponse* à un message précédent (pour fermer un dialogue), le système émetteur doit y inclure un MRN. La valeur de ce MRN doit être exactement égale à la valeur du MIN du message auquel on répond `[1]`.

**Séquence d'exemple d'un dialogue complet :**

1. **Requête Pilote :** L'avion envoie `DM 6 REQUEST FL350`. Le système avion lui assigne le **MIN = 8** `[1]`.
2. **Attente ATC :** Le sol répond `UM 1 STANDBY`. Le système sol lui assigne le **MIN = 12**, et comme c'est une réponse au DM 6, il inclut le **MRN = 8** `[1]`.
3. **Clairance ATC :** Le sol envoie `UM 20 CLIMB TO FL350`. Le système sol incrémente son compteur et lui assigne le **MIN = 13**, et inclut toujours le **MRN = 8** pour lier cette clairance à la requête initiale `[1]`.
4. **Acceptation Pilote :** L'avion répond `DM 0 WILCO`. Le système avion incrémente son compteur et lui assigne le **MIN = 9**, et inclut le **MRN = 13** pour répondre spécifiquement à la clairance `[1]`.

### 3. Gestion des États ("Open" vs "Closed")

Votre système doit suivre le cycle de vie de chaque transaction (Dialogue) pour déterminer si une réponse est toujours attendue.

* **Message Ouvert (Open) :** Un message contenant au moins un élément exigeant une réponse reste "Ouvert" tant que la réponse de clôture appropriée n'a pas été reçue `[1]`.
* **Message Fermé (Closed) :** Un message est fermé s'il ne requiert aucune réponse, ou si une réponse de "fermeture" a été reçue `[1]`.
* **Exception Critique (Standby) :** Les messages `UM 1 STANDBY` et `UM 2 REQUEST DEFERRED` (envoyés par le sol), ainsi que `DM 2 STANDBY` (envoyé par l'avion) **ne ferment pas** le message initial `[1]`. Le dialogue reste ouvert dans l'attente d'une réponse finale (Wilco, Unable, etc.) `[1]`.

### 4. Les Attributs de Réponse (Response Attributes)

Chaque élément de message défini par la norme possède un "Attribut de Réponse" qui dicte strictement au système (et à l'interface utilisateur) quelles réponses sont autorisées pour clôturer l'échange.

* **W/U (Wilco / Unable) :** Utilisé pour les clairances et instructions d'exécution.
* *Réponses valides :* `WILCO`, `UNABLE`, `STANDBY`, `NOT CURRENT DATA AUTHORITY`, `ERROR` `[1]`.
* *Comportement :* `WILCO` et `UNABLE` ferment le message `[1]`.


* **A/N (Affirm / Negative) :** Utilisé pour les questions fermées (ex: "Pouvez-vous accepter le niveau 370?").
* *Réponses valides :* `AFFIRM`, `NEGATIVE`, `STANDBY`, `ERROR` `[1]`.


* **R (Roger) :** Utilisé pour les informations ou les accusés de réception d'avis (advisories).
* *Réponses valides :* `ROGER`, `UNABLE`, `STANDBY`, `ERROR` `[1]`.


* **Y (N'importe quel message) :** Utilisé quand la réponse requise est une donnée (ex: "REPORT POSITION").
* *Réponses valides :* N'importe quel message CPDLC descendant ou ascendant (selon la direction de la requête) contenant la donnée attendue ferme le message `[1]`.


* **N (Aucune réponse) :** Le message est considéré comme fermé dès son émission/réception (hors acquittements techniques LACK si ATN B1) `[1]`.
* **NE (Not Enabled - Spécifique FANS 1/A) :** Indique qu'une réponse opérationnelle pourrait être attendue, mais que le système ne permet pas à l'humain de la sélectionner directement (le système ferme techniquement le message immédiatement) `[1]`.

### 5. Règle de Priorité (Precedence) pour les Messages Multi-Éléments

C'est une règle de conception fondamentale : l'humain ne peut fournir **qu'une seule réponse de fermeture** qui s'appliquera de manière atomique à l'ensemble du bloc de messages `[1]`. On ne peut pas accepter une partie d'une clairance et refuser l'autre ; c'est tout ou rien `[1]`.

Pour déterminer quelle réponse globale l'interface doit exiger, le système évalue la priorité (precedence) des attributs de tous les éléments du message, dans cet ordre décroissant :

1. **W/U** (Priorité la plus haute)
2. **A/N**
3. **R**
4. **Y**
5. **N** (Priorité la plus basse) `[1]`.

*Logique d'implémentation :* Si le sol envoie `UM 20 CLIMB TO FL350` (Attribut W/U) concaténé avec `UM 129 REPORT MAINTAINING FL350` (Attribut Y/R), l'attribut global du message devient **W/U** `[1]`. L'interface de l'avion ne doit proposer que WILCO, UNABLE ou STANDBY au pilote `[1]`. S'il répond WILCO, cela implique qu'il accepte la montée ET qu'il s'engage à faire le report une fois établi.

### 6. Timers et Acquittements Techniques

Pour éviter qu'une clairance ne reste bloquée et ne soit exécutée trop tardivement, des timers stricts doivent être implémentés :

* **LACK (Logical Acknowledgement) :** En environnement ATN B1, chaque message opérationnel déclenche automatiquement un accusé technique (UM 227 ou DM 100) confirmant à la machine émettrice que le message a été bien reçu et présenté à l'humain `[1]`. Le timer LACK est généralement défini sur 40 secondes `[1]`.
* **Timer Réponse Pilote (ttr) :** Sur les systèmes ATN B1, le système avion lance un timer de 100 secondes à la réception d'un UM. Si le pilote ne sélectionne pas de réponse de fermeture dans ce délai, l'interface supprime les boutons de réponse, le système envoie une erreur au sol (`AIRSYSTEM TIME OUT`), et le dialogue est fermé techniquement `[1]`.
* **Timer Réponse Sol (ttr/tts) :** Lorsqu'un pilote envoie une requête, le système au sol dispose d'un timer (souvent configuré autour de 250 secondes). Si le contrôleur ne répond pas dans ce délai, le système au sol clôture le dialogue et envoie un message d'erreur `ATC TIME OUT - REPEAT REQUEST` à l'avion `[1]`.