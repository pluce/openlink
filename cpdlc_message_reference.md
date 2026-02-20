Voici la spécification fonctionnelle complète et détaillée de l'ensemble des messages du protocole CPDLC (Uplink et Downlink), basée sur les standards de l'OACI (Doc 4444 et manuel GOLD).

### 1. Liste des Arguments (Variables)

Les messages CPDLC sont préformatés et utilisent des variables pour transmettre les données spécifiques à la clairance. Voici la liste des arguments utilisés dans les messages ci-dessous :

* `[level]` / `[altitude]` : Niveau de vol ou altitude précise.
* `[speed]` : Vitesse (indiquée en Mach ou IAS).
* `[time]` : Heure exacte (généralement au format UTC).
* `[position]` : Point de cheminement (waypoint), coordonnées ou repère géographique.
* `[direction]` : Direction (ex: LEFT, RIGHT).
* `[degrees]` : Cap (heading) ou route (track) en degrés.
* `[specified distance]` / `[distance offset]` : Distance en milles nautiques (NM).
* `[route clearance]` : Clairance de route (séquence de points, de voies aériennes ou directes).
* `[procedure name]` : Nom d'une procédure publiée (SID, STAR, approche).
* `[unit name]` : Nom de l'installation ATC en phonétique (ex: PARIS CONTROL).
* `[facility designation]` : Identifiant OACI à 4 lettres du centre ATC.
* `[frequency]` : Fréquence radio (VHF ou HF).
* `[code]` : Code transpondeur SSR (4 chiffres).
* `[atis code]` : Lettre d'identification de l'ATIS.
* `[error information]` : Description technique d'une erreur système.
* `[free text]` : Texte libre (limité en caractères).



### 2. Clé des Attributs de Réponse

Chaque message impose une règle stricte quant à la réponse que le système autorise :

* **W/U** : *Wilco / Unable* (Exige une réponse d'acceptation ou de refus de la clairance).
* **A/N** : *Affirm / Negative* (Exige une réponse binaire Oui/Non).
* **R** : *Roger* (Exige un simple accusé de réception).
* **Y** : Réponse requise (Exige un message de données en retour, ex: un report de position).
* **N** : Aucune réponse opérationnelle requise (le message se ferme dès réception).
* **NE** : *Not Enabled* (Spécifique FANS : la réponse est désactivée pour l'équipage, le système ferme le message immédiatement).



---

### 3. Messages Montants (Uplink Messages - UM) : Sol vers Air

#### Réponses, Accusés et Gestion de Connexion

| ID | Message avec [arguments] | Intention d'usage | Rép. | FANS | ATN B1 |
| --- | --- | --- | --- | --- | --- |
| UM 0 | UNABLE | L'ATC ne peut pas satisfaire la demande | N / NE | Oui | Oui |
| UM 1 | STANDBY | Demande reçue, évaluation en cours (délai court) | N / NE | Oui | Oui |
| UM 2 | REQUEST DEFERRED | Demande différée (délai long) | N / NE | Oui | Non |
| UM 3 | ROGER | Message reçu et compris | N / NE | Oui | Oui |
| UM 4 | AFFIRM | Oui (Affirmatif) | N / NE | Oui | Oui |
| UM 5 | NEGATIVE | Non (Négatif) | N / NE | Oui | Oui |
| UM 159 | ERROR [error information] | Message généré par le système notifiant une erreur | N / NE | Oui | Oui |
| UM 160 | NEXT DATA AUTHORITY [facility designation] | Désigne le prochain centre ATC pour le transfert | N / NE | Oui | Oui |
| UM 161 | END SERVICE | Termine la connexion CPDLC active | N / NE | Oui | Non |
| UM 162 | MESSAGE NOT SUPPORTED BY THIS ATS UNIT | Le centre ATC ne supporte pas le message reçu | N / NE | Oui (Texte) | Oui |
| UM 163 | [facility designation] | Notifie un identifiant ATSU | N / NE | Oui | Non |
| UM 211 | REQUEST FORWARDED | Demande transmise à l'autorité suivante | N | Oui (Texte) | Oui |
| UM 227 | LOGICAL ACKNOWLEDGEMENT | Accusé de réception technique (système) | N | Non | Oui |
| UM 233 | USE OF LOGICAL ACKNOWLEDGEMENT PROHIBITED | Désactive les accusés de réception techniques | N | Non | Non |
| UM 234 | FLIGHT PLAN NOT HELD | Le système sol n'a pas de plan de vol pour l'avion | N | Non | Non |
| UM 235 | ROGER 7500 | Accusé de réception d'interférence illicite | N | Non | Non |
| UM 237 | REQUEST AGAIN WITH NEXT UNIT | Veuillez refaire la demande au prochain centre | N | Oui (Texte) | Oui |

#### Clairances Verticales

| ID | Message avec [arguments] | Intention d'usage | Rép. | FANS | ATN B1 |
| --- | --- | --- | --- | --- | --- |
| UM 6 à 12 | EXPECT (diverses contraintes de niveau) | Notifie un futur changement de niveau. *À éviter (risque de confusion).* | R | Oui | Non |
| UM 13 à 18 | (Réservés ICAO Doc 4444) | - | - | Non | Non |
| UM 19 | MAINTAIN [level] | Maintenir le niveau spécifié | W/U | Oui | Oui |
| UM 20 | CLIMB TO [level] | Monter et maintenir le niveau spécifié | W/U | Oui | Oui |
| UM 21 | AT [time] CLIMB TO [level] | À l'heure spécifiée, monter au niveau | W/U | Oui | Non |
| UM 22 | AT [position] CLIMB TO [level] | Au point spécifié, monter au niveau | W/U | Oui | Non |
| UM 23 | DESCEND TO [level] | Descendre et maintenir le niveau spécifié | W/U | Oui | Oui |
| UM 24 | AT [time] DESCEND TO [level] | À l'heure spécifiée, descendre au niveau | W/U | Oui | Non |
| UM 25 | AT [position] DESCEND TO [level] | Au point spécifié, descendre au niveau | W/U | Oui | Non |
| UM 26 | CLIMB TO REACH [level] BY [time] | Monter pour atteindre le niveau avant l'heure | W/U | Oui | Oui |
| UM 27 | CLIMB TO REACH [level] BY [position] | Monter pour atteindre le niveau avant le point | W/U | Oui | Oui |
| UM 28 | DESCEND TO REACH [level] BY [time] | Descendre pour atteindre le niveau avant l'heure | W/U | Oui | Oui |
| UM 29 | DESCEND TO REACH [level] BY [position] | Descendre pour atteindre le niveau avant le point | W/U | Oui | Oui |
| UM 30 | MAINTAIN BLOCK [level] TO [level] | Maintenir un bloc d'altitude | W/U | Oui | Non |
| UM 31 | CLIMB TO AND MAINTAIN BLOCK [level] TO [level] | Monter et maintenir un bloc d'altitude | W/U | Oui | Non |
| UM 32 | DESCEND TO AND MAINTAIN BLOCK [level] TO [level] | Descendre et maintenir un bloc d'altitude | W/U | Oui | Non |
| UM 34 | CRUISE CLIMB TO [level] | Autorisation de montée en croisière | W/U | Oui | Non |
| UM 35 | CRUISE CLIMB ABOVE [level] | Montée en croisière au-dessus d'un niveau | W/U | Oui | Non |
| UM 36 | EXPEDITE CLIMB TO [level] | Monter au niveau avec le meilleur taux possible | W/U | Oui | Non |
| UM 37 | EXPEDITE DESCENT TO [level] | Descendre au niveau avec le meilleur taux | W/U | Oui | Non |
| UM 38 | IMMEDIATELY CLIMB TO [level] | Monter immédiatement (Urgence) | W/U | Oui | Non |
| UM 39 | IMMEDIATELY DESCEND TO [level] | Descendre immédiatement (Urgence) | W/U | Oui | Non |
| UM 171 | CLIMB AT [vertical rate] MINIMUM | Monter avec un taux vertical minimum | W/U | Oui | Oui |
| UM 172 | CLIMB AT [vertical rate] MAXIMUM | Monter avec un taux vertical maximum | W/U | Oui | Oui |
| UM 173 | DESCEND AT [vertical rate] MINIMUM | Descendre avec un taux vertical minimum | W/U | Oui | Oui |
| UM 174 | DESCEND AT [vertical rate] MAXIMUM | Descendre avec un taux vertical maximum | W/U | Oui | Oui |
| UM 192 | REACH [level] BY [time] | Atteindre le niveau spécifié à l'heure donnée | W/U | Non | Non |
| UM 209 | REACH [level] BY [position] | Atteindre le niveau spécifié au point donné | W/U | Non | Non |
| UM 219 | STOP CLIMB AT [level] | Arrêter la montée au niveau spécifié | W/U | Non | Non |
| UM 220 | STOP DESCENT AT [level] | Arrêter la descente au niveau spécifié | W/U | Non | Non |

#### Contraintes de Croisement et de Route

| ID | Message avec [arguments] | Intention d'usage | Rép. | FANS | ATN B1 |
| --- | --- | --- | --- | --- | --- |
| UM 46 | CROSS [position] AT [level] | Croiser le point au niveau spécifié | W/U | Oui | Oui |
| UM 47 | CROSS [position] AT OR ABOVE [level] | Croiser le point au niveau spécifié ou au-dessus | W/U | Oui | Oui |
| UM 48 | CROSS [position] AT OR BELOW [level] | Croiser le point au niveau spécifié ou en dessous | W/U | Oui | Oui |
| UM 49 | CROSS [position] AT AND MAINTAIN [level] | Croiser le point au niveau et le maintenir | W/U | Oui | Non |
| UM 50 | CROSS [position] BETWEEN [level] AND [level] | Croiser le point entre deux niveaux | W/U | Oui | Non |
| UM 51 | CROSS [position] AT [time] | Croiser le point à l'heure spécifiée | W/U | Oui | Oui |
| UM 52 | CROSS [position] AT OR BEFORE [time] | Croiser le point à l'heure spécifiée ou avant | W/U | Oui | Oui |
| UM 53 | CROSS [position] AT OR AFTER [time] | Croiser le point à l'heure spécifiée ou après | W/U | Oui | Oui |
| UM 54 | CROSS [position] BETWEEN [time] AND [time] | Croiser le point dans l'intervalle de temps | W/U | Oui | Oui |
| UM 55 | CROSS [position] AT [speed] | Croiser le point à la vitesse spécifiée | W/U | Oui | Oui |
| UM 56 | CROSS [position] AT OR LESS THAN [speed] | Croiser le point à la vitesse ou moins | W/U | Oui | Non |
| UM 57 | CROSS [position] AT OR GREATER THAN [speed] | Croiser le point à la vitesse ou plus | W/U | Oui | Non |
| UM 61 | CROSS [position] AT AND MAINTAIN [level] AT [speed] | Croiser le point au niveau et à la vitesse donnés | W/U | Oui | Oui |
| UM 73 | [departure clearance] | Clairance de départ (DCL) | W/U | Oui | Non |
| UM 74 | PROCEED DIRECT TO [position] | Procéder directement vers le point spécifié | W/U | Oui | Oui |
| UM 75 | WHEN ABLE PROCEED DIRECT TO [position] | Dès que possible, procéder direct vers le point | W/U | Oui | Non |
| UM 76 | AT [time] PROCEED DIRECT TO [position] | À l'heure donnée, procéder direct vers le point | W/U | Oui | Non |
| UM 77 | AT [position] PROCEED DIRECT TO [position] | Au point A, procéder direct vers le point B | W/U | Oui | Non |
| UM 78 | AT [level] PROCEED DIRECT TO [position] | En atteignant le niveau, procéder direct au point | W/U | Oui | Non |
| UM 79 | CLEARED TO [position] VIA [route clearance] | Autorisé vers le point via la route spécifiée | W/U | Oui | Oui |
| UM 80 | CLEARED [route clearance] | Autorisé via la route spécifiée | W/U | Oui | Oui |
| UM 81 | CLEARED [procedure name] | Autorisé pour la procédure (SID/STAR/App) | W/U | Oui | Non |
| UM 82 | CLEARED TO DEVIATE UP TO [specified distance][direction] OF ROUTE | Autorisé à dévier de la route spécifiée | W/U | Oui | Oui |
| UM 83 | AT [position] CLEARED [route clearance] | Au point donné, autorisé via la route | W/U | Oui | Non |
| UM 84 | AT [position] CLEARED [procedure name] | Au point donné, autorisé pour la procédure | W/U | Oui | Non |
| UM 91 | HOLD AT [position] MAINTAIN [level] INBOUND TRACK [degrees][direction] TURNS [leg type] | Entrer dans un circuit d'attente spécifique | W/U | Oui | Non |
| UM 92 | HOLD AT [position] AS PUBLISHED MAINTAIN [level] | Entrer dans l'attente publiée au point donné | W/U | Oui | Oui |

#### Changements de Cap, Vitesse et Décalage Latéral (Offset)

| ID | Message avec [arguments] | Intention d'usage | Rép. | FANS | ATN B1 |
| --- | --- | --- | --- | --- | --- |
| UM 64 | OFFSET [specified distance][direction] OF ROUTE | Voler en décalage parallèle à la route | W/U | Oui | Oui |
| UM 67 | PROCEED BACK ON ROUTE | Rejoindre la route autorisée | W/U | Oui | Non |
| UM 94 | TURN [direction] HEADING [degrees] | Tourner à gauche/droite au cap spécifié | W/U | Oui | Oui |
| UM 95 | TURN [direction] GROUND TRACK [degrees] | Tourner à gauche/droite sur la route sol spécifiée | W/U | Oui | Non |
| UM 96 | CONTINUE PRESENT HEADING | Continuer sur le cap actuel | W/U | Oui | Oui |
| UM 190 | FLY HEADING [degrees] | Voler au cap spécifié | W/U | Oui | Oui |
| UM 215 | TURN [direction][degrees] DEGREES | Tourner d'un nombre défini de degrés | W/U | Oui | Oui |
| UM 106 | MAINTAIN [speed] | Maintenir la vitesse spécifiée | W/U | Oui | Oui |
| UM 107 | MAINTAIN PRESENT SPEED | Maintenir la vitesse actuelle | W/U | Oui | Oui |
| UM 108 | MAINTAIN [speed] OR GREATER | Maintenir la vitesse spécifiée ou plus | W/U | Oui | Oui |
| UM 109 | MAINTAIN [speed] OR LESS | Maintenir la vitesse spécifiée ou moins | W/U | Oui | Oui |
| UM 110 | MAINTAIN [speed] TO [speed] | Maintenir la vitesse dans la plage donnée | W/U | Oui | Non |
| UM 111 | INCREASE SPEED TO [speed] | Augmenter la vitesse à la valeur spécifiée | W/U | Oui | Non |
| UM 113 | REDUCE SPEED TO [speed] | Réduire la vitesse à la valeur spécifiée | W/U | Oui | Non |
| UM 115 | DO NOT EXCEED [speed] | Ne pas dépasser la vitesse spécifiée | W/U | Oui | Non |
| UM 116 | RESUME NORMAL SPEED | Reprendre la vitesse normale | W/U | Oui | Oui |
| UM 222 | NO SPEED RESTRICTION | Aucune restriction de vitesse | R | Oui (Texte) | Oui |

#### Demandes de Contact, Surveillance et Rapports

| ID | Message avec [arguments] | Intention d'usage | Rép. | FANS | ATN B1 |
| --- | --- | --- | --- | --- | --- |
| UM 117 | CONTACT [unit name][frequency] | Contacter l'ATC sur la fréquence spécifiée | W/U | Oui | Oui |
| UM 118 | AT [position] CONTACT [unit name][frequency] | Au point donné, contacter l'ATC sur la fréquence | W/U | Oui | Non |
| UM 119 | AT [time] CONTACT [unit name][frequency] | À l'heure donnée, contacter l'ATC sur la fréquence | W/U | Oui | Non |
| UM 120 | MONITOR [unit name][frequency] | Veiller (écouter) la fréquence spécifiée | W/U | Oui | Oui |
| UM 123 | SQUAWK [code] | Afficher le code transpondeur spécifié | W/U | Oui | Oui |
| UM 124 | STOP SQUAWK | Désactiver le transpondeur | W/U | Oui | Non |
| UM 179 | SQUAWK IDENT | Appuyer sur le bouton IDENT du transpondeur | W/U | Oui | Oui |
| UM 127 | REPORT BACK ON ROUTE | Signaler le retour sur la route autorisée | W/U / R | Oui | Non |
| UM 128 | REPORT LEAVING [level] | Signaler le franchissement (départ) du niveau | W/U / R | Oui | Non |
| UM 129 | REPORT MAINTAINING [level] | Signaler le maintien (mise en palier) au niveau | W/U / R | Oui | Non |
| UM 130 | REPORT PASSING [position] | Signaler le passage au point spécifié | W/U / R | Oui | Non |
| UM 132 | REPORT POSITION | Envoyer un rapport de position complet | Y/NE | Oui | Non |
| UM 133 | REPORT PRESENT LEVEL | Signaler le niveau actuel | Y/NE | Oui | Oui |
| UM 134 | REPORT [speed type] SPEED | Signaler la vitesse actuelle | Y/NE/R | Oui | Non |
| UM 135 | CONFIRM ASSIGNED LEVEL | Confirmer le niveau actuellement assigné | Y/NE | Oui | Oui |
| UM 137 | CONFIRM ASSIGNED ROUTE | Confirmer la route actuellement assignée | Y/NE | Oui | Non |
| UM 147 | REQUEST POSITION REPORT | Requête explicite de rapport de position | Y/NE | Oui | Non |
| UM 148 | WHEN CAN YOU ACCEPT [level] | Quand pouvez-vous accepter le niveau spécifié? | Y/NE | Oui | Oui |
| UM 149 | CAN YOU ACCEPT [level] AT [position] | Pouvez-vous accepter le niveau à ce point? | A/N | Oui | Non |

#### Informations, Négociations et Textes Libres

| ID | Message avec [arguments] | Intention d'usage | Rép. | FANS | ATN B1 |
| --- | --- | --- | --- | --- | --- |
| UM 153 | ALTIMETER [altimeter] | Calage altimétrique (QNH) | R | Oui | Non |
| UM 154 | RADAR SERVICE TERMINATED | Service radar terminé | R | Oui | Non |
| UM 157 | CHECK STUCK MICROPHONE [frequency] | Vérifier qu'un micro n'est pas bloqué | N/R | Oui | Oui |
| UM 158 | ATIS [atis code] | Notifie le code ATIS actuel | R | Oui | Non |
| UM 164 | WHEN READY | À exécuter "quand vous serez prêt" | N/NE | Oui | Non |
| UM 165 | THEN | Sert à lier deux instructions (Ensuite...) | N/NE | Oui | Oui |
| UM 166 | DUE TO TRAFFIC | (Raison) En raison du trafic | N/NE | Oui | Non |
| UM 167 | DUE TO AIRSPACE RESTRICTION | (Raison) En raison d'une restriction d'espace | N/NE | Oui | Non |
| UM 168 | DISREGARD | Ignorez la dernière communication | R | Oui | Non |
| UM 176 | MAINTAIN OWN SEPARATION AND VMC | Maintenez votre propre séparation et VMC | W/U | Oui | Non |
| UM 177 | AT PILOTS DISCRETION | À la discrétion du pilote | N | Oui | Non |
| UM 169-170, 183 | [free text] | Messages en texte libre (différents niveaux d'urgence) | Varie | Oui | Oui |

---

### 4. Messages Descendants (Downlink Messages - DM) : Air vers Sol

#### Réponses Opérationnelles

| ID | Message avec [arguments] | Intention d'usage | Rép. | FANS | ATN B1 |
| --- | --- | --- | --- | --- | --- |
| DM 0 | WILCO | Instruction comprise et sera exécutée | N | Oui | Oui |
| DM 1 | UNABLE | Impossible de se conformer à l'instruction | N | Oui | Oui |
| DM 2 | STANDBY | Attendez, nous évaluons la demande | N | Oui | Oui |
| DM 3 | ROGER | Message bien reçu | N | Oui | Oui |
| DM 4 | AFFIRM | Oui | N | Oui | Oui |
| DM 5 | NEGATIVE | Non | N | Oui | Oui |
| DM 62 | ERROR [error information] | Erreur technique détectée par l'avionique | N | Oui | Oui |
| DM 63 | NOT CURRENT DATA AUTHORITY | Le centre n'est pas l'autorité de données actuelle | N | Oui | Oui |
| DM 100 | LOGICAL ACKNOWLEDGEMENT | Accusé de réception logique/technique (ATN) | N | Non | Oui |
| DM 107 | NOT AUTHORIZED NEXT DATA AUTHORITY | Le centre n'est pas autorisé comme prochain ATSU | N | Non | Oui |

#### Requêtes du Pilote

| ID | Message avec [arguments] | Intention d'usage | Rép. | FANS | ATN B1 |
| --- | --- | --- | --- | --- | --- |
| DM 6 | REQUEST [level] | Demande le niveau spécifié | Y | Oui | Oui |
| DM 7 | REQUEST BLOCK [level] TO [level] | Demande un bloc d'altitude | Y | Oui | Non |
| DM 9 | REQUEST CLIMB TO [level] | Demande une montée au niveau spécifié | Y | Oui | Oui |
| DM 10 | REQUEST DESCENT TO [level] | Demande une descente au niveau spécifié | Y | Oui | Oui |
| DM 11 | AT [position] REQUEST CLIMB TO [level] | Demande à monter au point spécifié | Y | Oui | Non |
| DM 12 | AT [position] REQUEST DESCENT TO [level] | Demande à descendre au point spécifié | Y | Oui | Non |
| DM 15 | REQUEST OFFSET [specified distance][direction] OF ROUTE | Demande un décalage parallèle | Y | Oui | Non |
| DM 18 | REQUEST [speed] | Demande la vitesse spécifiée | Y | Oui | Oui |
| DM 20 | REQUEST VOICE CONTACT | Demande un contact vocal avec le contrôleur | Y | Oui | Non |
| DM 22 | REQUEST DIRECT TO [position] | Demande une route directe vers le point spécifié | Y | Oui | Oui |
| DM 24 | REQUEST [route clearance] | Demande une modification complète de route | Y | Oui | Non |
| DM 25 | REQUEST CLEARANCE | Demande la clairance de départ/océanique | Y | Oui | Non |
| DM 26 | REQUEST WEATHER DEVIATION TO [position] VIA [route clearance] | Demande une déviation météo structurée | Y | Oui | Non |
| DM 27 | REQUEST WEATHER DEVIATION UP TO [specified distance][direction] OF ROUTE | Demande une déviation météo (offset) | Y | Oui | Oui |
| DM 49 à 54 | WHEN CAN WE EXPECT... | Demande de délais d'anticipation pour diverses clairances | Y | Oui | Non |
| DM 69 | REQUEST VMC DESCENT | Demande de descente à vue (VMC) | Y | Oui | Non |
| DM 70 | REQUEST HEADING [degrees] | Demande un cap spécifique | Y | Oui | Non |

#### Rapports et Notifications

| ID | Message avec [arguments] | Intention d'usage | Rép. | FANS | ATN B1 |
| --- | --- | --- | --- | --- | --- |
| DM 28 | LEAVING [level] | Quitte le niveau (Rapport) | N | Oui | Non |
| DM 29 | CLIMBING TO [level] | En montée vers le niveau (Rapport) | N | Oui | Non |
| DM 30 | DESCENDING TO [level] | En descente vers le niveau (Rapport) | N | Oui | Non |
| DM 31 | PASSING [position] | Passage du point spécifié (Rapport) | N | Oui | Non |
| DM 32 | PRESENT LEVEL [level] | Le niveau actuel est... | N | Oui | Oui |
| DM 33 | PRESENT POSITION [position] | La position actuelle est... | N | Oui | Non |
| DM 34 | PRESENT SPEED [speed] | La vitesse actuelle est... | N | Oui | Non |
| DM 37 | MAINTAINING [level] | En palier au niveau spécifié | N | Oui | Non |
| DM 38 | ASSIGNED LEVEL [level] | Confirme le niveau assigné | N | Oui | Oui |
| DM 39 | ASSIGNED SPEED [speed] | Confirme la vitesse assignée | N | Oui | Non |
| DM 40 | ASSIGNED ROUTE [route clearance] | Confirme la route assignée | N | Oui | Non |
| DM 41 | BACK ON ROUTE | De retour sur la route autorisée | N | Oui | Non |
| DM 48 | POSITION REPORT [position report] | Rapport de position complet | N | Oui | Non |
| DM 65 | DUE TO WEATHER | (Explication) Raison météorologique | N | Oui | Oui |
| DM 66 | DUE TO AIRCRAFT PERFORMANCE | (Explication) Limite de performance de l'avion | N | Oui | Oui |
| DM 89 | MONITORING [unit name][frequency] | Confirme la veille sur la fréquence spécifiée | N | Oui (Texte) | Oui |

#### Urgences et Textes Libres

| ID | Message avec [arguments] | Intention d'usage | Rép. | FANS | ATN B1 |
| --- | --- | --- | --- | --- | --- |
| DM 55 | PAN PAN PAN | Urgence absolue (Urgency) | Y/N | Oui | Oui |
| DM 56 | MAYDAY MAYDAY MAYDAY | Détresse absolue (Distress) | Y/N | Oui | Oui |
| DM 57 | [remaining fuel] OF FUEL REMAINING AND [persons on board] PERSONS ON BOARD | Informations de survie (Carburant et Âmes à bord) | Y/N | Oui | Oui |
| DM 58 | CANCEL EMERGENCY | Annule la déclaration d'urgence | Y/N | Oui | Oui |
| DM 59 | DIVERTING TO [position] VIA [route clearance] | Déroutement d'urgence vers le point spécifié | Y/N | Oui | Oui |
| DM 60 | OFFSETTING [specified distance][direction] OF ROUTE | Décalage d'urgence hors de la route | Y/N | Oui | Oui |
| DM 61 | DESCENDING TO [level] | Descente d'urgence | Y/N | Oui | Oui |
| DM 67, 68, 98 | [free text] | Texte libre saisi par l'équipage | Varie | Oui | Oui |
| DM 81-86 | WE CAN / CANNOT ACCEPT... | Réponses aux négociations ATC | N | Oui | Non |

Note technique : Les messages identifiés comme "Non" dans l'une des colonnes FANS ou ATN B1 déclencheront automatiquement une erreur de type `MESSAGE NOT SUPPORTED` (UM 162) s'ils sont envoyés sur le mauvais réseau.






Voici un document récapitulatif basé sur le manuel **GOLD (Global Operational Data Link Document)**. Il détaille les messages descendants (**Downlinks**) ayant un attribut de réponse "**Y**" et les messages montants (**Uplinks**) spécifiques qui peuvent y répondre pour clore le dialogue.

---

# Récapitulatif des Dialogues CPDLC (Réponse de type "Y")

En CPDLC, l'attribut de réponse **"Y"** signifie qu'une réponse opérationnelle est requise pour clore le message. Pour un message descendant (DM) envoyé par le pilote, cela signifie que le contrôleur doit répondre avec un message montant (UM) spécifique pour que la transaction soit considérée comme terminée.

### 1. Requêtes de Niveau (Vertical Requests)

Ces messages concernent les demandes de changement d'altitude ou de niveau de vol.

| Message Pilote (Downlink) | Réponses possibles du Contrôleur (Uplink) |
| --- | --- |
| <br>**DM 6** : REQUEST [level] 

 | <br>**Clôture directe :** UM 0 (UNABLE), UM 19 (MAINTAIN), UM 20 (CLIMB TO), UM 23 (DESCEND TO), UM 26/27 (CLIMB TO REACH BY), UM 28/29 (DESCEND TO REACH BY), UM 46 (CROSS AT), UM 47 (CROSS AT OR ABOVE), UM 48 (CROSS AT OR BELOW).

 |
| <br>**DM 9** : REQUEST CLIMB TO [level] 

 | Identiques à DM 6.

 |
| <br>**DM 10** : REQUEST DESCENT TO [level] 

 | Identiques à DM 6.

 |
| <br>**DM 7** : REQUEST BLOCK [level] TO [level] 

 | Messages de clairance de bloc ou de niveau.

 |

**Note importante :** L'envoi de **UM 1 (STANDBY)** ou **UM 2 (REQUEST DEFERRED)** par le contrôleur indique que la demande est reçue mais ne clôture pas techniquement le dialogue.

### 2. Requêtes de Route et de Cap (Lateral Requests)

Messages utilisés pour modifier la trajectoire latérale de l'avion.

| Message Pilote (Downlink) | Réponses possibles du Contrôleur (Uplink) |
| --- | --- |
| <br>**DM 22** : REQUEST DIRECT TO [position] 

 | <br>**Clôture :** UM 0 (UNABLE), UM 74 (PROCEED DIRECT TO), UM 96 (CONTINUE PRESENT HEADING), UM 190 (FLY HEADING).

 |
| <br>**DM 27** : REQUEST WEATHER DEVIATION UP TO [distance] [direction] 

 | <br>**Clôture :** UM 0 (UNABLE), UM 82 (CLEARED TO DEVIATE), UM 64 (OFFSET), UM 74 (DIRECT TO), UM 96 (PRESENT HEADING), UM 190 (FLY HEADING).

 |

### 3. Requêtes de Vitesse (Speed Requests)

| Message Pilote (Downlink) | Réponses possibles du Contrôleur (Uplink) |
| --- | --- |
| <br>**DM 18** : REQUEST [speed] 

 | <br>**Clôture :** UM 0 (UNABLE), UM 106 (MAINTAIN SPEED), UM 107 (PRESENT SPEED), UM 108 (SPEED OR GREATER), UM 109 (SPEED OR LESS), UM 116 (RESUME NORMAL SPEED), UM 222 (NO SPEED RESTRICTION).

 |

### 4. Demandes d'Information (Negotiation Requests)

Ces messages sont utilisés pour connaître les capacités futures ou les attentes de clairance.

| Message Pilote (Downlink) | Réponses possibles du Contrôleur (Uplink) |
| --- | --- |
| <br>**DM 52** : WHEN CAN WE EXPECT LOWER [level] 

 | <br>**Clôture :** UM 9 (EXPECT DESCENT AT [time]) ou UM 10 (EXPECT DESCENT AT [position]).

 |
| <br>**DM 53** : WHEN CAN WE EXPECT HIGHER [level] 

 | <br>**Clôture :** UM 7 (EXPECT CLIMB AT [time]) ou UM 8 (EXPECT CLIMB AT [position]).

 |
| <br>**DM 49** : WHEN CAN WE EXPECT [speed] 

 | <br>**Clôture :** UM 100 (AT [time] EXPECT [speed]) ou UM 101 (AT [position] EXPECT [speed]).

 |

---

### Points de vigilance pour le système (Gestion technique)

* 
**Réponse de clôture** : Un dialogue est "ouvert" tant qu'un message de la série n'a pas reçu sa réponse de clôture opérationnelle (comme WILCO ou UNABLE).


* 
**Messages rejetés** : Si le contrôleur envoie une réponse de clôture (ex: une clairance) après qu'un délai technique (timer) a expiré, le système avionique peut rejeter le message avec une erreur.


* 
**Messages système additionnels** : Pour les systèmes **ATN B1**, les messages comme **UM 162 (SERVICE UNAVAILABLE)** ou **UM 237 (REQUEST AGAIN WITH NEXT UNIT)** sont également utilisés pour clore les requêtes pilotes qu'une unité ne peut pas traiter.



Souhaitez-vous des précisions sur les temps de réponse (timers) associés à ces messages "Y" ?