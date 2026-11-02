# Audit initial — Talents, spécialisations et méta-progression Survivors

> Date : 2026-11-02  
> Base auditée : branche `main`, commit `8bd57975570ac0f8e1a7b1a0b4ea4f4cafd8aa37`.  
> Portée : audit statique du dépôt et de ses adaptateurs de données Legion. Aucun gameplay, profil ou protocole n'a été modifié.

## Résumé exécutif

Le dépôt possède déjà les principaux points d'extension nécessaires à une refonte :

- un parseur Legion qui lit `ChrSpecialization`, `Talent`, `Spell` et `SpellMisc` ;
- un modèle Legion qui représente naturellement sept rangées de trois talents ;
- un adaptateur qui injecte des onglets dans le catalogue de talents historique ;
- un catalogue Survivors `AbilityDef` avec des comportements de simulation explicites ;
- un profil persistant avec talents, gold, perks, achievements et récompenses ;
- une offre de cartes qui pioche les aptitudes à partir du pool de la classe.

Mais l'intégration est actuellement incomplète et aplatie à plusieurs endroits. Les principaux constats sont :

1. `benilla-legion::classes` sait lire des spécialisations et leurs 21 sièges de talents, mais `entities/legion.rs` les convertit en talents à trois rangs (ou un rang au dernier palier) dans le modèle Vanilla.
2. Seules les classes d'identifiants `[6, 10, 12]` sont actuellement traitées comme classes Legion.
3. Le profil stocke les talents par nom de classe, pas par spécialisation.
4. Les sorts Legion chargés via cette intégration fournissent des noms et des icônes ; le comportement Survivors reste une définition manuelle `AbilityDef`.
5. L'arbre Survivors applique principalement des bonus statistiques à partir du thème et de la position du talent, sans brancher les talents sur des transformations ou déblocages de sorts.

Le dépôt n'inclut pas les tables du client Legion comme corpus de données versionné. L'inventaire complet des spécialisations et talents réels de l'installation doit donc être finalisé sur une machine disposant d'une installation 7.3.5 valide et des définitions DBD requises. Le code permet de lire ces sources mais l'audit statique ne peut pas certifier que chaque table est présente et complète sur la machine de l'utilisateur.

## 1. Matrice des sources

| Source | Ce qu'elle fournit | Ce qu'elle ne fournit pas dans le flux actuel |
|---|---|---|
| `benilla-legion/src/classes.rs` | `ChrSpecialization` (id, classe, ordre, nom, icône) ; `Talent` (id, rangée, colonne, SpellID) ; `Spell` (nom) ; `SpellMisc` (icône) | Description d'effet de talent, règles d'activation Survivors, coefficient de dégâts, recharge et ciblage |
| `benilla-legion/src/library.rs` | API `Library::specs(class)` et `Library::spell(id)` basée sur les tables DBD/DB2 | Ne garantit pas que les tables soient présentes dans toute installation/configuration |
| `entities/legion.rs` | Adaptation des spécialisations et talents dans `TalentCatalog` historique ; icônes des classes Legion | Modèle de choix par rangée distinct, stockage de sélection `SpecId`, comportement du sort |
| `survivors/data.rs` | Classes, pools et `AbilityDef` codés pour le mode ; table de noms/icônes de sorts Legion | Construction automatique d'un comportement Survivors depuis un simple SpellID |
| `survivors/talents.rs` | Règles de talents Vanilla (rang, prérequis, points par palier, bonus) et thèmes de stats pour les noms de specialization | Un modèle fidèle de choix exclusif Legion (un talent choisi par rangée) |
| `survivors/profile.rs` | Sauvegarde persistante par TOML, talents, monnaie et méta-bonus | État par spécialisation ; nœuds de maîtrise ; actif de classe |
| `survivors/offer.rs` | Pool des nouvelles aptitudes et améliorations proposées en run | Filtrage dynamique par déblocages d'une spécialisation |
| `survivors/abilities.rs` | Exécution des types d'aptitudes Survivors et lancement d'effets visuels | Exécution générique de n'importe quel sort issu de Spell.db2 |
| `survivors/net/*` | Réplication des données de run, états des joueurs, actions et effets | Un contrat dédié aux nouveaux choix de spécialisation ou capacités actives |
| `character.rs` | Interface de talents actuelle : trois arbres, points, rangs, bonus | Interface de choix par spécialisation avec chemins, jalons et prévisualisation |

### Dépendances locales

- `WOW_LEGION` ouvre une copie locale ou distante du client Legion.
- `WOW_LEGION_DBD` fournit les définitions DBD nécessaires à la lecture des tables DB2.
- La bibliothèque utilise `ChrSpecialization`, `Talent`, `Spell` et `SpellMisc` pour son catalogue de classes/sorts.
- Les archives de jeu doivent rester en lecture seule ; aucun fichier du client n'est à committer dans le dépôt.
- Les données de jeu et DBD n'étant pas incluses dans ce dépôt, le nombre exact de lignes et l'inventaire complet des talents d'une installation 7.3.5 ne sont pas certifiables par cet audit statique.

## 2. Spécialisations et talents Legion

### 2.1 Modèle bas niveau déjà présent

Dans `crates/benilla-legion/src/classes.rs` :

- `LegionSpec` : `id`, `order`, `name`, `icon`, `talents`.
- `LegionTalent` : `id`, `tier`, `column`, `spell`.
- `Classes::specs(class)` sélectionne les spécialisations d'une classe et les trie selon `OrderIndex`.
- `Classes::talents(class, spec)` rassemble les talents propres à la spécialisation et les talents partagés de classe (`SpecID == 0`), en donnant priorité au talent propre lorsqu'il occupe le même siège.
- Les talents sont rangés par couple `(TierID, ColumnIndex)`.

Le commentaire source décrit explicitement sept rangées (tiers 0 à 6) et trois colonnes (0 à 2). C'est une structure fidèle au format des choix de spécialisation, et non un système de rangs 1/3.

### 2.2 Adaptateur actuellement problématique

Dans `crates/benilla-app/src/entities/legion.rs` :

- `TALENT_BASE = 100_000` sert à éviter les collisions d'ID avec les talents Vanilla.
- `TALENT_RANKS = 3` est utilisé pour remplir les trois premiers rangs de chaque talent ordinaire avec le même SpellID.
- Le septième palier (`tier == 6`) est converti en un seul rang et marqué `exceptional`.
- `prereq_talent`, `prereq_rank` et `required_spell` sont mis à zéro lors de la conversion.
- L'appel `add_classes` boucle seulement sur `data::LEGION_CLASSES = [6, 10, 12]`.

Effet : un choix exclusif par rangée devient un talent à trois rangs dans l'ancien modèle. Le système de points peut alors laisser investir plusieurs fois dans le même siège, ce qui n'exprime pas le modèle Legion.

**Recommandation :** ne pas étendre ce contournement. Créer un modèle de données Legion distinct, avec une sélection d'un talent par rangée et des identifiants source conservés.

## 3. Couverture des classes et spécialisations

### Classes

`survivors/data.rs` définit 12 classes dans `CLASSES` : les classes originales et les trois classes Legion (Chevalier de la mort, Moine, Chasseur de démons).

Cependant :

- `LEGION_CLASSES` est codé en dur à `[6, 10, 12]`.
- `class_shown` considère uniquement ces trois classes comme conditionnées par les données Legion.
- `entities/legion.rs::add_classes` ne récupère les spécialisations que pour ces trois identifiants.

La bibliothèque Legion expose pourtant `Library::specs(class)` pour un identifiant de classe donné. Le chargement des spécialisations peut donc être généralisé après vérification des tables et des mappings d'ID.

### Spécialisations

Le code source sait lire des spécialisations pour n'importe quel identifiant de classe fourni à `Library::specs`, mais le chargement Survivors actuel n'appelle cette fonction que pour les trois classes ajoutées avec Legion. L'audit ne doit pas en déduire que toutes les spécialisations de chaque classe sont déjà exposées au joueur.

**À faire pendant l'audit runtime :** exécuter l'inventaire sur l'installation 7.3.5 et produire une matrice `class_id / spec_id / spec_name / icon_id / talent_count / missing_spell_ids`.

## 4. Sorts : données de référence et comportements Survivors

### 4.1 Ce qui est chargé automatiquement

`benilla-legion/src/classes.rs` charge :

- `Spell.Name_lang` ;
- `SpellMisc.SpellIconFileDataID` pour la difficulté de base ;
- `ChrSpecialization` ;
- `Talent`.

Dans `entities/legion.rs`, les sorts référencés par les pools codés de Survivors sont résolus par `Library::spell`, puis transmis à `data::set_legion_spells` comme noms et chemins d'icônes.

### 4.2 Ce qui reste codé manuellement

`survivors/data.rs::AbilityDef` contient le comportement Survivors :

- `Kind` (Bolt, Strike, Melee, Nova, Cone, Ground, GroundSelf, Chain, Dot, Aura, Orbit, Shield, Immune, Thorns, Heal, Totem) ;
- dégâts, dégâts périodiques, recharge, portée, rayon, nombre de cibles et durée ;
- effets (ralentissement, root, stun, fear, affaiblissement, drain, exécution) ;
- marquage, totem, choix d'effet visuel et paramètres de présentation.

`survivors/abilities.rs::cast_for` exécute ces types avec un comportement adapté au mode. Un SpellID Legion avec nom et icône n'a donc pas de comportement Survivors par défaut.

### 4.3 Pools d'aptitudes

`ClassDef::pool` définit les aptitudes qu'une classe peut obtenir. `survivors/offer.rs::pool` ajoute les aptitudes non possédées depuis ce pool lorsque le nombre de slots le permet, et propose des améliorations pour les sorts déjà acquis.

Il n'existe pas encore de filtre de pool basé sur une spécialisation ou sur des nœuds de maîtrise. C'est le point d'intégration privilégié pour les futurs déblocages, à condition que le choix soit porté par l'état autoritaire du joueur.

## 5. Progression persistante actuelle

### Profil

`survivors/profile.rs::Profile` sauvegarde notamment :

- `gold` ;
- `talent_points` ;
- `talents: BTreeMap<String, Spent>` ;
- `perks`, `claimed`, `zones`, `inventory`, `equipped`, `appearances`, `pets` et le Codex.

Les champs sont sous `#[serde(default)]`, ce qui permet aux profils plus anciens d'omettre des champs ajoutés avec des valeurs par défaut.

### Problème de portée des talents

`talent_points` est partagé par le profil. La carte `talents` stocke un `Spent` par nom de classe. La fonction `meta_bonuses` lit les points investis pour la classe, puis calcule des bonus à partir des onglets retournés par `TalentCatalog`.

Pour les classes à plusieurs spécialisations, ce format ne permet pas d'isoler proprement les choix de chaque spécialisation. Une clé stable composée de l'identifiant de classe et du `SpecId` sera nécessaire pour le nouveau système.

### Perks et récompenses

Les points de récompense sont gagnés via les achievements. Les perks actuels couvrent principalement rerolls, banishes, gold, XP et ouverture de run. Les perks compagnon, collection, équipement et cosmétiques forment d'autres branches persistantes.

Ils constituent une base de méta-progression de compte, mais ne représentent pas encore une progression identitaire par spécialisation.

## 6. Multijoueur et autorité

Le mode Survivors utilise un host autoritaire : il exécute la simulation et transmet l'état des héros, les changements et les effets. Les invités transmettent leurs commandes de joueur et le host les valide.

Les futurs choix de spécialisation et actifs ne doivent pas être uniquement des états d'interface locaux.

À vérifier/implémenter lorsque les changements gameplay commenceront :

- sérialiser les choix de spécialisation dans le loadout transporté par le joueur ;
- valider côté host qu'un sort est débloqué et disponible ;
- synchroniser l'activation, le cooldown et les effets de l'actif de classe ;
- garantir que le host et l'invité calculent les mêmes bonus ;
- couvrir la reconnexion et la resynchronisation.

Cet audit n'a pas modifié le protocole.

## 7. Écart fonctionnel

| Besoin cible | État actuel | Écart |
|---|---|---|
| Sept rangées, trois choix exclusifs | Parser Legion le représente ; adaptateur le convertit en rangs | Modèle de domaine à séparer |
| Progression indépendante par spécialisation | Profil par classe | Clé de sauvegarde à revoir |
| Archétype de gameplay par spec | Bonus statistiques par thème de talent | Définition des identités manquante |
| Déblocage de sorts depuis talents | Pool par classe | Filtrage par spécialisation et déblocages manquants |
| Transformation de sorts | Perks de cartes augmentent des paramètres | Modificateurs explicites de comportement manquants |
| Actif de classe | Pas de contrat général de capacité active dans la progression | Système gameplay/HUD/network à définir |
| Progression de compte liée aux classes | Achievements, perks et collectibles génériques | Objectifs de maîtrise et récompenses thématiques manquants |
| Sources 7.3.5 complètes | Lecture DBD/DB2 externe disponible | Inventaire runtime non exécuté dans cet environnement |

## 8. Matrice d'inventaire runtime à compléter

Cette matrice est le livrable attendu sur une machine qui dispose de l'installation Legion et des définitions DBD. Elle ne doit pas être remplie par supposition.

| Champ | Source | Statut statique |
|---|---|---|
| Classe / ClassID | `ChrSpecialization.ClassID` | Parser prêt ; chargement limité à 3 classes |
| Spécialisation / SpecID | `ChrSpecialization.ID` | Parser prêt |
| Nom | `ChrSpecialization.Name_lang` | Parser prêt |
| Ordre | `ChrSpecialization.OrderIndex` | Parser prêt |
| Icône de spécialisation | `ChrSpecialization.SpellIconFileID` | Parser prêt |
| Talent / TalentID | `Talent.ID` | Parser prêt |
| Rangée / TierID | `Talent.TierID` | Parser prêt |
| Colonne / ColumnIndex | `Talent.ColumnIndex` | Parser prêt |
| Sort associé / SpellID | `Talent.SpellID` | Parser prêt |
| Nom du sort | `Spell.Name_lang` | Parser prêt |
| Icône du sort | `SpellMisc.SpellIconFileDataID` | Parser prêt |
| Description/effets détaillés du sort | Tables supplémentaires Spell/Effect/Visual selon build | Non chargés par le catalogue de classe actuel |
| Comportement Survivors | `survivors/data.rs::AbilityDef` | Définition manuelle nécessaire |

## 9. Recommandations de conception

1. **Séparer deux modèles de talent.** Garder les talents Vanilla à rangs et prérequis ; créer un modèle Legion à choix exclusif par tier.
2. **Faire porter la progression par la spécialisation.** La sauvegarde doit être indexée par classe et spec.
3. **Ne pas confondre SpellID et AbilityKey.** Le premier vient des données client ; le second désigne un comportement du mode.
4. **Créer un catalogue d'adaptation.** Une table explicite relie `SpellID` à `AbilityKey`, aux paramètres de gameplay et aux effets pris en charge.
5. **Séparer déblocage et équilibrage.** Un talent source peut informer le nom, l'icône et le thème ; son effet Survivors doit être conçu et testé.
6. **Définir les archétypes après l'inventaire des sorts.** Éviter de définir un archétype avant de savoir quelles capacités et variantes sont réellement disponibles.
7. **Garder la méta-progression de compte distincte.** L'héritage/domaine de classe peut réutiliser les achievements, le Codex et les rewards, sans convertir toutes les ressources en une seule monnaie.

## 10. Critères de fin de Phase 1

- [x] Cartographie statique du code, des adaptateurs et des points d'intégration.
- [x] Identification du modèle Legion natif sept tiers/trois choix.
- [x] Identification de l'aplatissement en trois rangs dans l'adaptateur.
- [x] Identification du stockage des talents uniquement par classe.
- [x] Identification des limites du catalogue de sorts (nom/icône, comportement Survivors manuel).
- [ ] Inventaire runtime complet des classes/spécialisations/talents à partir d'une installation 7.3.5.
- [ ] Rapport des talents dont le SpellID est absent/incomplet.
- [ ] Rapport des effets de sorts, talents et traits d'artefact pouvant être adaptés au gameplay Survivors.

Les trois derniers points nécessitent une exécution avec les données locales `WOW_LEGION` et `WOW_LEGION_DBD`. Ils restent explicitement ouverts, plutôt que d'être remplacés par une liste théorique non vérifiée.

## Références externes — mécanique historique de Legion 7.3.x

Ces références servent à confirmer le contexte de conception du Creuset et ne remplacent pas l'inventaire local DB2 :

- [Blizzard — Patch 7.3 Preview: Netherlight Crucible](https://news.blizzard.com/en-us/article/20979712/patch-7-3-preview-netherlight-crucible) : choix Lumière/Ombre et traits reliques conditionnés par la progression de l'arme.
- [Wowhead — The Netherlight Crucible and You](https://www.wowhead.com/guide/the-netherlight-crucible-and-you-5424) : structure des trois paliers de relique et conditions historiques.
- [Wowhead — Legion 7.3.5 Artifact guide](https://www.wowhead.com/guide/classes/warlock/affliction/artifact) : exemple d'arme prodigieuse et des choix du Creuset.

Les archétypes proposés dans le document de conception restent des choix propres à Survivors ; ils ne sont pas présentés comme les noms ou effets des talents historiques.
