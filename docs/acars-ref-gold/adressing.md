Dans le système CPDLC (qu'il soit basé sur FANS 1/A ou ATN B1), l'adressage est un mécanisme hybride qui repose sur des bases de données embarquées, l'action initiale du pilote, et des échanges automatisés entre les centres de contrôle.

Voici comment les deux parties s'identifient et se trouvent sur le réseau :

1. Comment l'avion connaît l'adresse du centre de contrôle (Sol)
L'avion n'a pas besoin de connaître l'adresse technique réseau du sol par défaut, il utilise un système d'alias.

L'action du pilote : Pour initier la toute première connexion, le pilote saisit manuellement un code de connexion (Logon Address) de 4 lettres dans son MCDU ou DCDU. Ce code correspond généralement à l'identifiant OACI du centre de contrôle (par exemple, KZNY pour New York ou LFFF pour Paris). Les autorités publient ces codes de connexion officiels dans leurs publications d'information aéronautique (AIP) et sur les cartes de navigation.

La traduction par l'avionique (FANS 1/A sur ACARS) : L'ordinateur de communication de l'avion (le CMU) contient une base de données d'adressage interne mise à jour par la compagnie aérienne. Lorsqu'il lit le code de 4 lettres, le CMU le convertit en une adresse ACARS de 7 caractères requise par le réseau de données (par exemple, FCCC est traduit par l'adresse réseau BZVCAYA pour le centre de Brazzaville).

La traduction par l'avionique (ATN B1) : De manière similaire, pour l'espace européen, l'avionique contient un registre d'adresses NSAP (Network Service Access Point) très complexes. Le code de 4 lettres saisi par le pilote permet à l'avionique de pointer vers la bonne adresse NSAP du centre ATN.

2. Comment le centre de contrôle connaît l'adresse de l'avion
Le centre de contrôle ne devine jamais l'adresse d'un avion à l'avance ; c'est l'avion qui doit d'abord se manifester.

Le message de Logon (La poignée de main) : Lorsque l'avionique initie la demande de connexion (le message FN_CON en FANS ou CM_LOGON_REQUEST en ATN), le message sortant inclut l'adresse technique unique de l'aéronef.

Les données transmises : Ce message contient l'immatriculation de l'avion, son adresse OACI 24-bits (attribuée de manière unique à la cellule de l'avion), et l'adresse spécifique de son application CPDLC (comme le TSAP en ATN).

La corrélation avec le plan de vol : Le système au sol reçoit cette demande. Avant d'accepter d'envoyer des messages à cette adresse technique, l'ordinateur du contrôle aérien vérifie que les données reçues correspondent exactement au plan de vol déposé dans le système. Une fois la corrélation validée, le système au sol mémorise l'adresse technique ACARS ou ATN de l'avion et l'associe à la trace radar du contrôleur.

3. Les transferts en vol (Handoffs)
Une fois la première connexion établie, le pilote n'a plus à saisir d'adresses. Le système devient totalement transparent pour l'équipage grâce à deux mécanismes :

Le transfert Sol-Sol (Address Forwarding) : Avant que l'avion ne quitte un secteur, le centre de contrôle actuel transmet automatiquement l'adresse technique de l'avion et ses informations de connexion au centre de contrôle suivant via le réseau terrestre (protocoles AIDC ou OLDI).

Le message de contact Air-Sol (typiquement `UM 117 CONTACT`) : Si les centres ne sont pas interconnectés au sol, le centre de contrôle actuel envoie un message de contact à l'avion contenant la prochaine autorité à joindre. L'avionique exécute alors la demande de connexion de manière totalement invisible pour le pilote.