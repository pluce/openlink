Voici une spécification fonctionnelle détaillée pour le processus d'identification (Logon) et la gestion des sessions CPDLC, basée sur le manuel GOLD (Global Operational Data Link Document) de l'OACI, conçue pour votre équipe de développement.

### 1. Concepts Fondamentaux

* **CDA (Current Data Authority) :** L'ATSU (centre de contrôle) avec lequel l'avion a une connexion CPDLC **active** `[1]`. L'avion ne peut avoir qu'une seule connexion active à la fois `[1]`. Seul le CDA est autorisé à échanger des messages de contrôle avec le pilote `[1]`.
* **NDA (Next Data Authority) :** L'ATSU désigné par le CDA actuel pour prendre le relais. L'avion peut avoir une seule connexion **inactive** avec ce centre en préparation d'un transfert de secteur `[1]`.
* **Séparation Logon / Connexion :** L'avion initie toujours le Logon (application DLIC ou AFN), mais c'est toujours le système au sol qui initie la connexion CPDLC proprement dite `[1]`.

### 2. Phase 1 : L'Identification (Logon - DLIC)

**Déclencheur :** Le pilote initie le Logon entre 10 et 25 minutes avant d'entrer dans l'espace aérien géré par liaison de données `[1]`.

* **Message Avion -> Sol :** L'avion envoie un `Logon Request` (CM_LOGON_REQUEST en ATN ou FN_CON en FANS) `[1]`.
* *Contenu obligatoire :* Identifiant de l'ATSU cible (ex: KZNY), Identification du vol (identique à la case 7 du plan de vol, ex: AFR123), Immatriculation de l'avion ou adresse 24-bits OACI (ex: F-GZTA), et généralement les aéroports de départ et de destination `[1]`.


* **Traitement Sol :** Le système au sol (ATSU) reçoit la requête et effectue une corrélation avec le plan de vol déposé `[1]`. Les données doivent correspondre exactement. Si l'immatriculation contient un espace ou un tiret, la logique du serveur doit le nettoyer pour éviter un rejet automatique de la corrélation `[1]`.
* **Message Sol -> Avion :** Le sol renvoie un `Logon Response` (CM_LOGON_RESPONSE ou FN_AK) `[1]`. S'il est positif, l'avion est maintenant "identifié" sur le réseau de ce centre, mais **pas encore connecté** au CPDLC.

### 3. Phase 2 : Établissement de la Connexion CPDLC

**Déclencheur :** Après un Logon réussi, le système au sol décide d'ouvrir la session (soit par une action du contrôleur, soit automatiquement lorsque l'avion approche de la frontière) `[1]`.

* **Message Sol -> Avion :** L'ATSU envoie un `Connection Request` (CPDLC_START_REQUEST ou CR1) `[1]`.
* **Logique Avion (Machine à états) :**
* *Cas A (Aucune connexion existante) :* L'avion accepte la demande, établit cette connexion comme **Active** (le centre devient le CDA), et répond par un `Connection Confirm` `[1]`.
* *Cas B (Une connexion active existe déjà avec un autre centre) :* L'avion vérifie si l'ATSU demandeur a été préalablement désigné comme NDA (Next Data Authority) par le centre actif actuel `[1]`.
* *Si oui :* L'avion accepte la requête, établit la connexion comme **Inactive** et répond par un `Connection Confirm` `[1]`.
* *Si non :* L'avion rejette formellement la requête en envoyant un `Connection Rejection` avec le code d'erreur `NOT AUTHORIZED NEXT DATA AUTHORITY` (DM 107) ou `NOT CURRENT DATA AUTHORITY` (DM 63) `[1]`.





### 4. Phase 3 : Transfert de Connexion (Handoff)

Ce processus garantit que la responsabilité de l'avion passe de manière fluide et sécurisée d'un centre à l'autre `[1]`.

* **Étape 1 (Notification du NDA) :** Le CDA actuel envoie un message montant `NEXT DATA AUTHORITY` (UM 160) contenant le code OACI de 4 caractères du prochain ATSU `[1]`. L'avion stocke cette information `[1]`.
* **Étape 2 (Logon Forwarding) :** Le CDA transmet les informations de Logon de l'avion au prochain ATSU (généralement de manière transparente via le réseau sol-sol) ou demande à l'avion de s'identifier auprès du nouveau centre via un message de contact standard `UM 117 CONTACT` `[1]`.
* **Étape 3 (Connexion Inactive) :** Le prochain ATSU envoie son `Connection Request`. Puisqu'il est le NDA reconnu (grâce à l'Étape 1), l'avion l'accepte en tant que connexion **Inactive** en arrière-plan `[1]`.
* **Étape 4 (Terminaison) :** Au moment de franchir le secteur, le CDA envoie un message `Termination Request` (UM 161 END SERVICE), souvent concaténé avec un ordre de changement de fréquence vocale (ex: UM 117 CONTACT) `[1]`.
* **Étape 5 (Bascule Active) :** Le pilote accepte le transfert (DM 0 WILCO) `[1]`. Le système de l'avion envoie automatiquement un `Termination Confirmation`, coupe la connexion avec l'ancien CDA, et **promeut la connexion inactive en connexion active** `[1]`. Le prochain ATSU devient le nouveau CDA et peut désormais envoyer des clairances `[1]`.

### 5. Phase 4 : Gestion des Erreurs et Déconnexions Anormales

* **Messages non autorisés :** Si un ATSU disposant de la connexion inactive (ou n'ayant aucune connexion) tente d'envoyer un message opérationnel (ex: "CLIMB TO FL350"), l'avionique le rejette automatiquement et envoie le message `NOT CURRENT DATA AUTHORITY` (DM 63) au sol `[1]`.
* **Changement de route imprévu :** Si l'avion subit un re-routing, le CDA peut envoyer un nouveau message `NEXT DATA AUTHORITY` (UM 160) avec un centre différent `[1]`. L'avionique écrase l'ancien NDA et déconnecte immédiatement la session inactive de l'ancien centre s'il s'était déjà connecté `[1]`.
* **Abortion de session :** Si un message de terminaison `Termination Request` est reçu alors que des dialogues de clairance (Uplinks) sont toujours ouverts en attente d'une réponse pilote, certaines avioniques purgent toutes les connexions (User Abort) par sécurité `[1]`. Le système sol doit donc s'assurer qu'il n'y a plus de requêtes en attente avant d'initier la déconnexion `[1]`.

### Séquence Récapitulative du Transfert de Vol (Handoff Nominal)

| Émetteur | Destinataire | Action / Contenu du Message | Phase |
| --- | --- | --- | --- |
| Avion | ATSU 1 | Initie la session (Logon Request) | DLIC |
| ATSU 1 | Avion | Confirme le Logon (Logon Response) | DLIC |
| ATSU 1 | Avion | Demande de connexion (Connection Request) | CPDLC |
| Avion | ATSU 1 | Confirme la connexion (L'ATSU 1 devient le CDA) | CPDLC |
| ... | ... | ... Échanges CPDLC Opérationnels... | CPDLC |
| ATSU 1 | Avion | Désigne le prochain centre (UM 160 NDA = ATSU 2) | CPDLC |
| ATSU 1 | ATSU 2 | Transmet le Logon de l'avion au prochain centre | Sol-Sol |
| ATSU 2 | Avion | Demande de connexion (Connection Request) | CPDLC |
| Avion | ATSU 2 | Confirme la connexion (L'ATSU 2 devient le NDA) | CPDLC |
| ATSU 1 | Avion | Fin de service / Chgt Fréquence (UM 161 + UM 117) | CPDLC |
| Avion | ATSU 1 | Accepte le transfert (DM 0 WILCO) | CPDLC |
| Avion | ATSU 1 | Confirmation de fin de connexion (Terminated) | CPDLC |
| Avion | Interne | Promeut l'ATSU 2 en tant que CDA (Bascule Active) | Interne |