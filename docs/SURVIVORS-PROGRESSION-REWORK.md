# Survivors — Refonte des talents et de la méta-progression

> Statut : conception et audit initial. Aucun changement de gameplay n'est inclus dans cette étape.
> Référence de données : WoW Legion 7.3.5, complétée par les systèmes historiques de WoW Vanilla lorsque cela sert le mode Survivors.

## Objectif

Faire évoluer la progression Survivors d'une accumulation principalement statistique vers une progression qui donne une identité mécanique aux classes et spécialisations, crée des jalons mémorables et donne des objectifs persistants à long terme.

Le joueur ne doit pas seulement rendre son personnage plus puissant : il doit pouvoir le faire évoluer vers une manière différente de jouer.

## Principes de conception

1. **Les spécialisations définissent le gameplay.** Un choix de spécialisation doit avoir des conséquences perceptibles sur les sorts proposés, les synergies, les priorités et les capacités disponibles.
2. **Les talents offrent des décisions, pas uniquement des pourcentages.** Les bonus statistiques restent utiles, mais les paliers importants doivent modifier un sort, créer une interaction ou débloquer une aptitude.
3. **Les actifs majeurs restent séparés des six emplacements ordinaires.** Un actif emblématique de classe/spécialisation pourra utiliser son propre emplacement et son propre contrat d'activation.
4. **La méta-progression doit respecter l'univers Warcraft.** Réutiliser les thèmes, noms, icônes, spécialisations, sorts, armes prodigieuses et récompenses réellement présents dans les données 7.3.5, sans présenter les concepts de design inventés comme des données officielles.
5. **La progression persistante doit rester lisible et réversible selon des règles explicites.** Les choix permanents, les choix de run et les déblocages de compte ne doivent pas être confondus.
6. **Le multijoueur doit rester déterministe.** Le host valide les choix et les effets de gameplay ; les clients affichent le même état. Toute nouvelle donnée portée par un joueur doit être incluse dans le loadout/snapshot approprié.
7. **Compatibilité ascendante des profils.** Les nouveaux champs sérialisés doivent avoir des valeurs par défaut. Tout changement de sens d'un champ existant exige une migration explicite.

## Les trois couches de progression proposées

### 1. Spécialisation — identité du personnage

- Une spécialisation sélectionnée explicitement par classe.
- Les spécialisations disponibles doivent provenir des données de classe réellement présentes dans l'installation Legion.
- La spécialisation définit un archétype de gameplay, un pool d'aptitudes et des synergies.
- Les classes Vanilla peuvent conserver leurs arbres historiques lorsque les données appropriées sont absentes ; elles ne doivent pas être artificiellement converties en spécialisations Legion.

### 2. Maîtrise — progression persistante de classe/spécialisation

- Représenter fidèlement les sept rangées de trois choix des talents de spécialisation Legion.
- Un choix par rangée, avec les éventuelles conditions d'accès et remplacements selon les données disponibles.
- Les nœuds peuvent être de plusieurs types : bonus statistique, modification d'une aptitude, interaction entre aptitudes, déblocage d'une aptitude, actif majeur.
- Les grands jalons (proposition : rangées 3/5/7, à équilibrer) doivent apporter une transformation perceptible plutôt qu'un simple bonus plus élevé.
- L'état de maîtrise doit être sauvegardé par classe **et** spécialisation, pas seulement par nom de classe.
- Le coût, le plafond et les conditions d'acquisition restent à définir après l'inventaire des données et un audit d'équilibrage.

### 3. Héritage / domaine de classe — progression de compte

- Une couche persistante transversale qui porte les objectifs de campagne, hauts faits, cosmétiques, titres et déblocages systémiques.
- Récompenser la maîtrise de classes et spécialisations, les victoires, les boss, les zones et les défis.
- Ne pas rendre tous les bonus de compte obligatoires pour être compétitif.
- Étudier une ressource thématique dédiée (nom de travail : « Échos de puissance »), sans la créer avant d'avoir défini ses sources, ses dépenses et son équilibre.
- Les perks actuels de compte, points de récompense, gold, équipement et collection restent distincts tant qu'une consolidation n'est pas justifiée.

## Références WoW 7.3.5 à réutiliser

- **Spécialisations et talents de classe :** `ChrSpecialization` et `Talent`, avec sept rangées de trois choix et les sorts associés.
- **Armes prodigieuses :** traits d'arme et identité spécifique à chaque spécialisation. À adapter en jalons de maîtrise, sans reprendre le grind d'Artifact Power tel quel.
- **Reliques et Creuset de Lumière-du-Néant :** exemple de progression en branches et de choix conditionnels. À utiliser comme inspiration, pas comme exigence de reproduction littérale.
- **Domaines de classe :** source d'identité, d'objectifs de campagne et de récompenses thématiques.
- **Sorts :** les tables Legion disponibles fournissent des identifiants, noms et icônes. Un sort n'est pas automatiquement jouable dans Survivors : son comportement, sa cible, ses dégâts, sa recharge et ses effets doivent être décrits dans le modèle Survivors.

Les mécanismes ci-dessus sont des références historiques. Les nouveaux archétypes, la structure de progression Survivors, les paliers majeurs et la ressource de méta-progression sont des propositions propres au projet.

## Contrat de données visé

Le modèle final devra distinguer au minimum :

- **ClassId** : classe.
- **SpecId** : spécialisation 7.3.5, quand elle existe.
- **TalentNodeId / SpellId** : identifiant source du nœud et sort associé, conservés sans les confondre.
- **NodeKind** : bonus, transformation, synergie, déblocage, actif.
- **UnlockRule** : prérequis de palier, choix exclusif, objectif ou progression persistante.
- **AbilityKey** : comportement Survivors concret, référencé par le nœud si celui-ci débloque ou modifie une aptitude.
- **ProgressionScope** : run, classe/spécialisation ou compte.
- **ProfileState** : choix persistants sérialisés avec des identifiants stables.

Aucun champ ou protocole ne doit être ajouté avant que les audits de compatibilité sauvegarde et multijoueur soient terminés.

## Phases de réalisation

### Phase 1 — Audit des données et inventaire (en cours)

- Cartographier les classes, spécialisations, talents et sorts disponibles dans le code et dans le chemin de chargement Legion.
- Distinguer les données sources du client 7.3.5, les adaptations déjà présentes et les éléments codés à la main pour Survivors.
- Identifier les lacunes : spécialisations non exposées, talents aplatis, sorts sans comportement Survivors, effets visuels manquants.
- Documenter les dépendances d'installation et les cas où les données locales sont indisponibles.
- Livrable : `docs/survivors-progression/AUDIT.md`, avec état des lieux, limitations et matrice des sources.

**Critère de sortie :** les sources, transformations et limites sont documentées ; chaque classe/spécialisation a un statut de disponibilité explicite. Aucun sort ou trait n'est inventé pour remplir un trou.

### Phase 2 — Modèle de spécialisation et de talents

- Définir un modèle de talents Legion séparé du modèle Vanilla à rangs multiples.
- Préserver les sept rangées et les trois choix par rangée.
- Identifier proprement les talents partagés de classe et les talents propres à une spécialisation.
- Stocker les choix par classe/spécialisation.
- Définir les règles de migration de profils.

**Critère de sortie :** tests unitaires des choix, exclusivités, déblocages et round-trip de sauvegarde ; aucune modification de gameplay non reliée.

### Phase 3 — Archétypes de gameplay

- Définir les archétypes à partir des spécialisations réellement disponibles.
- Associer chaque archétype à une boucle de jeu, des sorts centraux, des synergies et des faiblesses.
- Établir une matrice d'identité et d'équilibrage avant d'ajouter des chiffres.

**Critère de sortie :** chaque spécialisation a une identité jouable distincte et documentée.

### Phase 4 — Déblocages, transformations et actifs

- Connecter les nœuds aux aptitudes disponibles dans Survivors.
- Ajouter les déblocages au pool de cartes de niveau.
- Définir les transformations de sorts sans dupliquer inutilement les implémentations.
- Concevoir le contrat d'un actif de classe/spécialisation séparé des six slots ordinaires : activation, recharge, effets, réplication, HUD et accessibilité.

**Critère de sortie :** une aptitude débloquée est réellement utilisable, correctement affichée et synchronisée en multijoueur.

### Phase 5 — Méta-progression et héritage

- Définir les objectifs, récompenses, ressources et dépenses.
- Relier achievements, zones, boss et maîtrise des spécialisations.
- Conserver une progression de compte significative sans rendre les bonus numériques excessifs.
- Ajouter des récompenses cosmétiques et des déblocages de gameplay avec des conditions transparentes.

**Critère de sortie :** chaque boucle de récompense a une source, une dépense, un plafond et des tests de sauvegarde.

### Phase 6 — Interface de l'arbre

- Construire une interface de spécialisation lisible, avec chemins, prérequis, choix, paliers majeurs et prévisualisation.
- Différencier visuellement bonus, transformation, déblocage et actif.
- Afficher les noms, icônes, descriptions et sources des données disponibles.
- Prévoir anglais/français, résolution et navigation souris/clavier.

**Critère de sortie :** le joueur comprend les choix disponibles, les conséquences et les conditions de déblocage sans lire une documentation externe.

### Phase 7 — Équilibrage et validation

- Tester les spécialisations et leurs interactions avec les pools de cartes existants.
- Vérifier les sauvegardes anciennes et nouvelles.
- Vérifier host/guest, migration de loadout et re-synchronisation.
- Tester l'affichage et les effets visuels en jeu.
- Mesurer la puissance totale de la progression pour éviter une inflation de statistiques.

**Critère de sortie :** CI verte, tests gameplay/net/save au vert et validation visuelle en jeu.

## Garde-fous techniques

- La simulation et le host restent autoritaires.
- Ne pas coder un comportement de sort à partir de son seul nom ou de son icône.
- Ne pas traiter un talent Legion comme un sort à trois rangs par défaut.
- Ne pas mélanger talents Vanilla et talents Legion dans un modèle ambigu.
- Ne pas stocker la progression d'une spécialisation uniquement sous la clé du nom de classe.
- Ne pas ajouter une capacité active à la barre des six aptitudes ordinaires.
- Garder les données 7.3.5 externes et en lecture seule ; ne pas committer de fichiers de jeu.
- Tous les champs persistants nouveaux doivent être tolérants à l'absence dans les anciens profils.
- Toute nouvelle capacité multijoueur doit avoir une représentation validée côté host.

## Décisions encore ouvertes

- Quelles classes Vanilla doivent conserver leurs trois arbres historiques, et lesquelles doivent recevoir un modèle de spécialisations 7.3.5 si les tables le permettent ?
- Le choix de spécialisation est-il libre à chaque run, à la sélection du personnage, ou verrouillé par la progression ?
- Les talents de maîtrise sont-ils achetés avec une ressource persistante ou déverrouillés par objectifs ?
- Les actifs de classe sont-ils débloqués par la dernière rangée, par une quête/défi, ou par les deux ?
- Quelle proportion de la puissance doit venir de la progression persistante par rapport aux choix de la run ?

Ces décisions seront prises après la phase 1 et ne doivent pas être présupposées dans le code.
