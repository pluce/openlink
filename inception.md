Architecture de Référence pour un Système ACARS de Nouvelle Génération : OpenLink Aviation Network
Résumé Exécutif

L'écosystème de la simulation de vol a connu une transformation radicale au cours de la dernière décennie, portée par l'avènement de plateformes de simulation haute fidélité telles que Microsoft Flight Simulator (2020/2024) et X-Plane 12, ainsi que par une professionnalisation accrue des add-ons d'aéronefs. Au cœur de cet écosystème, les communications numériques entre le pilote, les opérations aériennes (AOC) et le contrôle du trafic aérien (ATC) reposent encore majoritairement sur le réseau "Hoppie ACARS". Bien que pionnier et fondamental pour la communauté, ce système accuse le poids de son architecture héritée du début des années 2000, basée sur un modèle de "polling" HTTP et des protocoles textuels ad-hoc.

Ce rapport technique propose une architecture de rupture, baptisée OpenLink, conçue pour répondre aux exigences de modernité, de performance temps réel, de sécurité et d'interopérabilité. L'analyse démontre que le passage à une Architecture Orientée Événements (EDA) est impératif. La solution recommandée s'articule autour du courtier de messages NATS JetStream, choisi pour sa capacité à gérer le multi-tenant de manière native, sa latence sub-milliseconde et sa persistance intégrée.

L'architecture OpenLink introduit trois innovations majeures :

Ségregation Stricte des Réseaux : Utilisation des "Accounts" NATS pour isoler hermétiquement les flux de données de VATSIM, IVAO et des compagnies aériennes virtuelles (VA), résolvant définitivement les problèmes de collision d'espaces de noms.

Identité Fédérée : Remplacement des codes de connexion statiques par une authentification OAuth2/OIDC dynamique, déléguant la confiance aux fournisseurs d'identité des réseaux (VATSIM Connect, IVAO Login) et assurant une non-répudiation forte.

Interopérabilité Standardisée : Adoption de schémas de données JSON stricts, inspirés des normes ARINC 633 et 623, transportés via WebSockets pour garantir une compatibilité totale avec les environnements contraints (WASM) des simulateurs modernes.

Ce document détaille l'implémentation technique de cette vision, depuis la topologie de l'infrastructure jusqu'aux spécifications des payloads JSON, offrant une feuille de route complète pour le développement d'un standard ouvert et pérenne.

1. Introduction et Analyse Contextuelle
1.1 L'Héritage ACARS dans la Simulation de Vol
Le système ACARS (Aircraft Communications Addressing and Reporting System) est l'épine dorsale des communications numériques dans l'aviation civile. Dans le monde virtuel, le réseau "Hoppie", développé par Jeroen Hoppenbrouwers, remplit ce rôle depuis plus de deux décennies. Il permet l'échange de messages TELEX, de requêtes CPDLC (Controller-Pilot Data Link Communications) et de rapports OOOI (Out-Off-On-In) entre les simulateurs de vol et les stations sol.   

Cependant, l'architecture de Hoppie a été conçue à une époque où le web était statique et les connexions intermittentes. Elle repose sur un modèle où le client (l'avion) doit interroger périodiquement le serveur central pour vérifier la présence de nouveaux messages. Ce mécanisme, suffisant pour quelques centaines d'utilisateurs, montre ses limites face à la croissance exponentielle de la communauté, qui compte désormais plus de 30 000 comptes actifs.   

1.2 Limitations Critiques de l'Architecture Actuelle
1.2.1 Latence et Inefficacité du Polling HTTP
Le protocole actuel oblige chaque client à effectuer une requête HTTP GET ou POST à intervalles réguliers (typiquement toutes les 15 à 60 secondes).   

Latence Induite : Un message urgent envoyé par un contrôleur (ex: "CLIMB IMMEDIATELY FL360") peut rester en attente sur le serveur jusqu'à la prochaine fenêtre de polling du client, introduisant un délai inacceptable pour des opérations tactiques.

Gaspillage de Ressources : Statistiquement, plus de 95% des requêtes de polling retournent une réponse vide ("No messages"). Cela génère un trafic réseau inutile et une charge CPU serveur disproportionnée pour gérer l'établissement et la fermeture des connexions TCP/TLS.

Le Phénomène du "Thundering Herd" : Lors d'événements majeurs comme le "Cross the Pond" sur VATSIM, des milliers de clients se connectent simultanément. Si le serveur ralentit, les clients ont tendance à réessayer plus agressivement, saturant davantage le système et provoquant des pannes en cascade.   

1.2.2 Sécurité et Gestion de l'Identité
L'authentification actuelle repose sur un "Logon Code" statique, généré une seule fois lors de l'inscription.   

Absence de Révocation : Si un code est compromis, il est difficile de le révoquer sans intervention manuelle.

Usurpation d'Identité : Rien n'empêche techniquement un utilisateur malveillant d'utiliser le code d'un autre pour envoyer des messages injurieux ou erronés.

Manque de Lien avec les Réseaux : Il n'existe pas de lien cryptographique entre un compte Hoppie et un CID VATSIM ou VID IVAO. Un utilisateur banni d'un réseau pour comportement inapproprié peut continuer à utiliser le système ACARS, car les identités sont découplées.

1.2.3 Problèmes de Concurrence et d'Espace de Noms
Le système actuel utilise un espace de noms plat pour les indicatifs (Callsigns).

Collisions : Un contrôleur connecté en tant que EGLL_GND sur IVAO et un autre sur VATSIM partagent le même identifiant dans le système Hoppie. Cela conduit à des situations où des messages destinés à l'un sont reçus par l'autre, ou pire, par les deux.   

Manque d'Isolation : Les tentatives de créer des sous-réseaux (VACC spécifiques, compagnies virtuelles) reposent souvent sur des conventions de nommage fragiles plutôt que sur une ségrégation technique réelle.

1.3 Définition des Objectifs de la Nouvelle Architecture
Pour concevoir un remplaçant viable, nous devons satisfaire les exigences suivantes, dérivées de l'analyse des besoins et des meilleures pratiques de l'industrie :

Architecture Événementielle (Event-Driven) : Abandonner le polling pour un modèle "Push". Le serveur doit notifier le client instantanément à l'arrivée d'un message.

Ségregation Multi-Tenant Native : Le système doit supporter plusieurs réseaux (VATSIM, IVAO, PilotEdge, VAs privées) sur la même infrastructure, avec une étanchéité totale des données.

Sécurité Fédérée (OAuth2) : L'authentification doit être déléguée aux fournisseurs d'identité de confiance (VATSIM Connect, IVAO API).

Compatibilité Technique (WASM/Web) : Le protocole de transport doit être compatible avec les environnements "sandboxés" des simulateurs modernes (Microsoft Flight Simulator via WASM) qui interdisent souvent les sockets TCP bruts.   

Standardisation des Données (JSON/ARINC) : Remplacer le texte libre par des structures de données typées et validables pour faciliter l'intégration par les développeurs tiers.

2. Philosophie Architecturale : Le Passage à l'Event Mesh
2.1 Le Concept d'Event Mesh
L'approche traditionnelle "Base de Données + API REST" est inadaptée aux communications temps réel. Nous proposons de basculer vers un Event Mesh (Maillage d'Événements). Dans ce modèle, l'infrastructure n'est plus un stockage passif, mais un système nerveux actif qui route les messages entre les producteurs (avions, dispatch) et les consommateurs (contrôleurs, cartes en direct) en temps réel.

2.2 Comparaison des Technologies de Messaging
Le choix du protocole de messagerie est la décision la plus critique de cette architecture. Nous avons évalué les trois standards dominants : MQTT, NATS, et AMQP (RabbitMQ).

Caractéristique	MQTT 5.0	NATS JetStream	RabbitMQ / AMQP	Analyse pour le cas d'usage ACARS
Modèle	Pub/Sub	Pub/Sub + Request/Reply + Streaming	Queue-based Routing	
NATS offre la flexibilité du Pub/Sub pour la diffusion (ATIS) et du Request/Reply pour les transactions (CPDLC).

Latence	Faible	Très Faible (< 100µs)	Moyenne	
NATS surpasse systématiquement MQTT et RabbitMQ en débit et latence.

Multi-Tenancy	Via ACLs (Complexe)	Natif (Accounts/Users)	Vhosts (Lourd)	
NATS gère l'isolation par "Accounts" de manière native et légère, idéal pour séparer VATSIM/IVAO.

Persistance	Retained Msgs (Limité)	Streams (Logs persistants)	Queues durables	NATS JetStream permet de stocker l'historique des messages (Store-and-Forward) sans base de données externe.
Transport Web	WebSockets	WebSockets	WebSockets (via plugin)	Égalité. Le support WebSocket est crucial pour le WASM MSFS.
Complexité Client	Faible	Très Faible	Élevée	Les clients NATS sont simples à implémenter, même dans des environnements contraints.
  
Décision : L'architecture OpenLink sera construite sur NATS JetStream. Ses capacités natives de multi-tenancy (Accounts), sa performance exceptionnelle sur du matériel modeste , et son support natif des WebSockets en font le candidat idéal pour remplacer l'infrastructure Hoppie.   

2.3 Topologie de l'Infrastructure
L'infrastructure physique sera distribuée pour assurer résilience et performance.

2.3.1 Le Cluster Central (Core)
Un cluster de 3 nœuds NATS JetStream assurera la persistance des données et le consensus (Raft).

Rôle : Stockage des messages (Streams), gestion des comptes, routage inter-cluster.

Localisation : Hébergement centralisé (ex: Europe ou US East) sur des instances VPS performantes mais économiques (NVMe requis pour JetStream).

2.3.2 Les Gateways Edge (Leaf Nodes)
Pour réduire la latence TCP (Round-Trip Time) pour les utilisateurs mondiaux, des "Leaf Nodes" NATS seront déployés géographiquement (Asie, Océanie, Amériques).

Fonctionnement : Un utilisateur à Sydney se connecte au Leaf Node "AU-East". Ce nœud maintient la connexion TCP locale et relaie les messages vers le cluster Core via une connexion optimisée.

Avantage : L'expérience utilisateur (connexion, handshake TLS) est instantanée, quelle que soit la distance avec le serveur central.

3. Sécurité et Identité : Une Approche "Zero Trust"
La sécurité est l'une des lacunes majeures du système actuel. OpenLink adopte une approche où l'infrastructure ne stocke aucun mot de passe ("Zero Trust Identity").

3.1 Intégration OAuth2 / OIDC
L'authentification est déléguée aux fournisseurs d'identité (IdP) existants : VATSIM Connect  et IVAO Login API.   

3.1.1 Le Flux d'Authentification (Workflow)
Initiation : L'utilisateur lance son client ACARS (ou l'avion dans le simulateur). Il clique sur "Connexion VATSIM".

Redirection : Une fenêtre de navigateur s'ouvre vers le service d'authentification OpenLink (auth.openlink.net), qui redirige vers auth.vatsim.net.

Consentement : L'utilisateur se connecte sur VATSIM et autorise l'application OpenLink à accéder à ses données (Scopes: full_name, vatsim_details, email).

Callback : VATSIM renvoie un code d'autorisation au service OpenLink.

Échange de Token : Le service OpenLink échange ce code contre un Access Token VATSIM et récupère le profil utilisateur (CID, Rating, Division).

Émission du Token NATS : C'est l'étape cruciale. Le service OpenLink génère un JWT NATS (User JWT) signé cryptographiquement. Ce token contient :

L'identité NATS (Subject) dérivée du CID (ex: vatsim_810000).

Les permissions Pub/Sub exactes (ex: droit de publier sur acars.uplink.810000 et souscrire à acars.downlink.810000).

L'expiration (ex: 24h).

Connexion : Le client reçoit ce JWT et l'utilise pour se connecter au serveur NATS via WebSocket.

3.2 Ségregation Multi-Tenant via NATS Accounts
L'utilisation des Accounts NATS permet de créer des silos étanches sur la même infrastructure physique.   

Structure des Comptes
Account NATS	Description	Population	Isolation
SYS	Système Interne	Services d'infra (Auth, Bridge, Metrics)	Accès total (Admin)
VATSIM	Réseau VATSIM	Utilisateurs authentifiés via VATSIM Connect	Isolés des autres réseaux. Peuvent voir les services globaux (Météo).
IVAO	Réseau IVAO	Utilisateurs authentifiés via IVAO API	Isolés des autres réseaux.
VA_Private	Compagnies Virtuelles	Utilisateurs avec double authentification (VA + Réseau)	Espace privé pour les opérations internes (Dispatch, Chat compagnie).
3.2.1 Exemple de Gestion des Collisions
Si l'utilisateur BAW123 existe sur VATSIM et IVAO simultanément :

Sur le compte NATS VATSIM, le client s'abonne au sujet acars.msg.BAW123.

Sur le compte NATS IVAO, un autre client s'abonne au sujet acars.msg.BAW123.

Grâce à l'isolation des comptes, un message envoyé à BAW123 dans le contexte VATSIM n'est jamais visible par le client IVAO, même si le nom du sujet est identique. Cela résout définitivement le problème de "crosstalk" observé sur Hoppie.   

3.3 Auth Callout : La Sécurité Dynamique
Pour gérer les permissions fines (ex: un contrôleur qui ouvre une position), nous utiliserons la fonctionnalité Auth Callout de NATS 2.10+.   

Au lieu de générer un JWT statique pour 24h, le serveur NATS peut interroger le service d'auth à chaque connexion.

Si un contrôleur change de position (passe de EGLL_GND à EGLL_TWR), il demande une reconnexion. Le service d'auth vérifie sa position actuelle sur le réseau (via l'API de données VATSIM/IVAO) et met à jour ses permissions d'abonnement en temps réel.

4. Architecture de Données : Standards et Schémas
L'une des plus grandes faiblesses du système actuel est l'utilisation de texte non structuré. OpenLink impose l'utilisation de JSON, validé par des schémas JSON Schema, et inspiré des standards aéronautiques ARINC.

4.1 Structure de l'Enveloppe (Envelope)
Tout message transitant sur le réseau OpenLink est encapsulé dans une enveloppe standardisée.

JSON
{
  "meta": {
    "id": "uuid-v4",
    "timestamp": "2026-02-08T20:30:00Z",
    "correlation_id": "uuid-request-original", // Pour le Request/Reply
    "version": "1.0"
  },
  "routing": {
    "source": "BAW123",
    "target": "EGLL_GND",
    "network": "VATSIM"
  },
  "type": "cpdlc.clearance.request",
  "payload": {... } // Contenu spécifique au type
}
4.2 Modernisation de l'ARINC 623 (ATC Applications)
L'ARINC 623 définit les messages ATS (Air Traffic Services) textuels. Nous transposons ces définitions en JSON.   

Exemple : Demande de Clairance de Départ (DCL)
Format Hoppie/TELEX Actuel (Texte brut) : REQ CLR TO KJFK VIA DVR1G FL340 (Parsing difficile, sujet à erreurs).

Format OpenLink JSON :

JSON
{
  "type": "cpdlc.dcl.request",
  "payload": {
    "flight_plan": {
      "origin": "EGLL",
      "destination": "KJFK",
      "flight_rule": "IFR"
    },
    "preferences": {
      "requested_level": 340,
      "requested_sid": "DVR1G"
    },
    "atis_code": "F",
    "parking_stand": "505",
    "aircraft_type": "A320"
  }
}
Exemple : Réponse de Clairance (DCL)
JSON
{
  "type": "cpdlc.dcl.response",
  "payload": {
    "status": "CLEARED",
    "clearance": {
      "departure_proc": "DVR1G",
      "initial_climb": { "alt": 6000, "unit": "FT" },
      "squawk": "4501",
      "frequency": "121.700",
      "next_event": "PUSHBACK"
    },
    "text_fallback": "CLEARED TO KJFK VIA DVR1G, CLIMB 6000FT, SQUAWK 4501, CONTACT GND 121.700"
  }
}
Avantage : Les add-ons sophistiqués (Fenix, PMDG) peuvent parser ce JSON pour :

Afficher la clairance formatée sur le DCDU.

Auto-tune la radio COM1 sur 121.700.

Auto-set le transpondeur sur 4501.

Imprimer le ticket via l'imprimante cockpit virtuelle.

4.3 Modernisation de l'ARINC 633 (AOC Data)
L'ARINC 633 définit les échanges entre l'avion et les opérations (AOC).   

Intégration SimBrief (OFP Push)
Au lieu que le pilote entre manuellement son ID SimBrief dans l'EFB, le système SimBrief (ou une VA) peut "pousser" le plan de vol directement dans la file d'attente de l'avion.

Sujet NATS : aoc.inbox.BAW123

Payload : Contient l'OFP complet en JSON (Route, Fuel, Charge, METARs).

Scénario : Le pilote allume les batteries. Le client ACARS se connecte. Il reçoit immédiatement l'événement aoc.flightplan.ready. L'EFB propose "Load Flight Plan?". Un clic, et tout est configuré.

5. Stratégie d'Intégration Client et Contraintes WASM
L'intégration dans les simulateurs est le défi technique le plus complexe, en particulier pour Microsoft Flight Simulator (MSFS).

5.1 La Contrainte du Sandbox WASM
MSFS exécute le code C++ des avions (PMDG, Fenix, FBW) dans un environnement WebAssembly (WASM).

Limitation : Le runtime WASM de MSFS bloque l'accès aux sockets TCP/UDP bruts (BSD Sockets) pour des raisons de sécurité, sauf via une API SimConnect limitée ou des extensions spécifiques. Cela rend impossible l'utilisation des clients NATS C++ standards qui attendent une connexion TCP pure.   

La Solution OpenLink : Utiliser WebSockets comme couche de transport universelle.

Les navigateurs (JS/TS dans les instruments HTML/CoherentGT) supportent nativement WebSocket.

Pour le code C++/Rust en WASM, il existe des bibliothèques capables de "tunneler" des communications via les APIs réseau autorisées de la plateforme ou via une couche JS intermédiaire (le "Gauge Bridge").

5.2 Architecture du SDK Client (LibOpenLink)
Pour faciliter l'adoption, nous ne pouvons pas demander à chaque développeur d'implémenter le protocole NATS complet. Nous proposerons un SDK officiel libopenlink.

Architecture du SDK
Core (Rust/C++) : Gestion de la machine à état, sérialisation JSON, et logique métier ACARS.

Transport Layer (Abstrait) : Interface interchangeable.

TcpTransport pour les applications Desktop (vPilot, Euroscope).

WebSocketTransport pour MSFS WASM et les EFBs web.

API Publique : Simple et orientée événement.

C++
// Exemple conceptuel d'utilisation du SDK en C++
auto acars = OpenLink::Client::Create("wss://api.openlink.net");

// Authentification
acars->Login(vatsim_token);

// Abonnement aux messages entrants
acars->OnMessage("cpdlc.dcl.response",(const Json& payload) {
    // Logique d'affichage sur le DCDU
    FMC::UplinkClearance(payload);
});

// Envoi d'une requête
acars->Send("cpdlc.dcl.request", request_payload);
5.3 Intégration avec les Outils Externes
SimBrief : Utilisation de Webhooks. SimBrief configure un webhook vers l'API OpenLink. À la génération d'un OFP, OpenLink le convertit en message NATS et le stocke dans le Stream du pilote.

Clients Pilotes (vPilot/xPilot) : Ces clients devront intégrer libopenlink. En attendant, un plugin ou un "proxy local" peut être utilisé.

Contrôleurs (Euroscope/Aurora) : Développement de plugins natifs utilisant le SDK C#. L'architecture événementielle permettra aux contrôleurs de voir le statut de connexion ACARS des pilotes en temps réel (Presence), ce qui est impossible actuellement avec Hoppie sans polling agressif.

6. Migration et Rétrocompatibilité (Le "Legacy Bridge")
Une migration "Big Bang" étant impossible, nous devons supporter les clients existants (Hoppie legacy) tout en incitant à la transition.

6.1 Le Proxy HTTP/1.1 (Pattern Adaptateur)
Le composant OpenLink Bridge expose une API HTTP identique à celle de Hoppie (http://www.hoppie.nl/acars/system/connect.html).

Fonctionnement :

Reçoit une requête de polling POST d'un client legacy (ex: FSLabs A320).

Authentifie l'utilisateur via son Logon Code (mappé temporairement vers un compte NATS).

Interroge le Stream NATS pour voir si des messages sont en attente.

Si oui, convertit le JSON OpenLink en format TELEX textuel.

Renvoie la réponse HTTP au client legacy.

Transparence : Pour l'avion legacy, rien n'a changé. Mais en coulisses, il bénéficie de la performance du backend NATS.

6.2 Stratégie de Transition
Phase 1 (Dual Run) : Le Bridge connecte les utilisateurs legacy au nouveau réseau. Les utilisateurs modernes (OpenLink natif) peuvent communiquer avec les utilisateurs legacy (conversion JSON <-> Texte automatique assurée par le Bridge).

Phase 2 (Incitation) : Les fonctionnalités avancées (Loadsheets binaires, Météo graphique, DCL riche) ne sont disponibles que sur le protocole natif.

Phase 3 (Deprecation) : Une fois 80% des clients majeurs migrés, le support legacy est progressivement réduit (rate-limiting).

7. Infrastructure et Scalabilité
7.1 Dimensionnement
Pour supporter 50 000 utilisateurs connectés (concurrency) et 1000 msgs/sec (débit moyen estimé) :

NATS Core : NATS est extrêmement efficient. Un seul nœud peut gérer des millions de messages par seconde. Pour la haute disponibilité (HA), un cluster de 3 nœuds est requis.

Spec : 3x VPS (4 vCPU, 8GB RAM, NVMe SSD). Le NVMe est crucial pour la performance des écritures JetStream (persistance).

Bande Passante : Le protocole binaire NATS est beaucoup plus léger que le XML/HTTP. La consommation estimée est négligeable par rapport aux coûts d'hébergement.

7.2 Résilience
JetStream Replication : Les messages (Streams) sont répliqués sur 3 nœuds (R=3). Si un serveur tombe, les données ne sont pas perdues et le service continue sans interruption (Leader Election automatique via Raft).

Load Balancing : Les connexions WebSocket entrantes sont réparties via un Load Balancer (ex: HAProxy ou Cloudflare) vers les nœuds du cluster ou les Leaf Nodes.

7.3 Coûts Opérationnels
Contrairement à une base de données relationnelle massive requise pour stocker l'historique, l'architecture Stream de NATS permet de définir des politiques de rétention (ex: "Garder les messages 24h ou 100MB max"). Cela permet de maintenir les coûts de stockage très bas. L'infrastructure complète peut être hébergée pour moins de 100€/mois, ce qui est soutenable pour une organisation communautaire financée par des dons.   

Conclusion et Recommandations
L'architecture OpenLink présentée dans ce rapport offre une solution pérenne, robuste et sécurisée pour remplacer le système Hoppie vieillissant. En s'appuyant sur :

NATS JetStream pour la performance événementielle et le multi-tenant,

OAuth2 pour la sécurité et la confiance,

WebSockets pour l'universalité d'accès (WASM),

JSON Schema pour la structure des données,

...cette proposition résout non seulement les problèmes techniques actuels (polling, latence, sécurité) mais ouvre la porte à des fonctionnalités inédites : télémesure temps réel, dispatch dynamique, et intégration ATC avancée.

Prochaines Étapes Recommandées :

Proof of Concept (PoC) : Déployer un cluster NATS de test et développer un client web minimal capable d'envoyer/recevoir un message structuré via WebSocket et authentification VATSIM Sandbox.

Formation du Groupe de Travail : Réunir les développeurs clés (FlyByWire, Fenix, vPilot, Swift) pour valider les schémas JSON (ARINC mapping).

Développement du Bridge : Prioriser la rétrocompatibilité pour assurer une adoption sans friction.

Cette modernisation est essentielle pour accompagner la prochaine génération de simulation de vol vers un niveau de réalisme et d'immersion opérationnelle inégalé.


hoppie.nl
Hoppie's ACARS
S'ouvre dans une nouvelle fenêtre

hoppie.nl
ACARS server API - Hoppie.nl
S'ouvre dans une nouvelle fenêtre

hoppie.nl
ACARS Project Progress - Hoppie.nl
S'ouvre dans une nouvelle fenêtre

hoppie.nl
Hoppie's ACARS Registration
S'ouvre dans une nouvelle fenêtre

reddit.com
VATSIM sending information through Hoppie? - Reddit
S'ouvre dans une nouvelle fenêtre

docs.nats.io
MQTT - NATS Docs
S'ouvre dans une nouvelle fenêtre

users.rust-lang.org
Possible to connect to `TcpListener` through WASM? - Rust Users Forum
S'ouvre dans une nouvelle fenêtre

hivemq.com
Building a Unified Namespace: Why MQTT Outperforms NATS - HiveMQ
S'ouvre dans une nouvelle fenêtre

streamnative.io
Pulsar vs. RabbitMQ vs. NATS JetStream - StreamNative
S'ouvre dans une nouvelle fenêtre

onidel.com
NATS JetStream vs RabbitMQ vs Apache Kafka on VPS in 2025 - Onidel
S'ouvre dans une nouvelle fenêtre

docs.nats.io
Multi Tenancy using Accounts - NATS Docs
S'ouvre dans une nouvelle fenêtre

docs.nats.io
nats bench | NATS Docs
S'ouvre dans une nouvelle fenêtre

vpsbenchmarks.com
Best VPS 2024 under $25 - VPSBenchmarks
S'ouvre dans une nouvelle fenêtre

vatsim.dev
Connect / OAuth2 - VATSIM.dev
S'ouvre dans une nouvelle fenêtre

vatsim.dev
APIs - VATSIM.dev
S'ouvre dans une nouvelle fenêtre

wiki.ivao.aero
Developers Website | IVAO Documentation Library
S'ouvre dans une nouvelle fenêtre

wiki.ivao.aero
API Documentation v2 - IVAO Wiki
S'ouvre dans une nouvelle fenêtre

docs.nats.io
Auth Callout | NATS Docs
S'ouvre dans une nouvelle fenêtre

aviation-ia.sae-itc.com
623-3 Character-Oriented Air Traffic Service (ATS) Applications - Specification - ARINC IA
S'ouvre dans une nouvelle fenêtre

sae.org
ARINC623-3 : 623-3 Character-Oriented Air Traffic Service (ATS) Applications
S'ouvre dans une nouvelle fenêtre

sae.org
633-4 AOC Air-Ground Data and Message Exchange Format ARINC633-4
S'ouvre dans une nouvelle fenêtre

bytron.aero
Understanding ARINC 633 format & EFF - Bytron Aviation Systems
S'ouvre dans une nouvelle fenêtre

github.com
Blazor webassembly using using NATS.NET · Issue #383 - GitHub