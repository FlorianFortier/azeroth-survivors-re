# Legion 7.3.5 — audit des sorts déclenchés et de leurs chaînes visuelles

> Build : 7.3.5.26972.  
> Provenance : rapports locaux fournis par le mainteneur depuis son installation CASC et les définitions WoWDBDefs locales.  
> Aucun asset, DB2/DBC ou CSV complet n'est versionné dans le dépôt.

## Résumé vérifié

| Mesure | Résultat |
|---|---:|
| Sorts déclenchés en entrée | 17 |
| Sorts déclenchés secondaires supplémentaires | 0 |
| Union des sorts traités | 17 |
| Effets de base | 24 |
| Arêtes parent → enfant | 17 |
| Lignes de détails | 24 |
| Lignes de chaîne visuelle | 91 |
| Références non résolues | 0 |
| Sorts avec au moins une entrée SpellXSpellVisual | 8 / 17 |
| SpellVisual | 8 |
| SpellVisualEvent | 16 |
| SpellVisualKit | 16 |
| SpellVisualKitEffect | 21 |
| SpellVisualKitModelAttach | 13 |
| SpellVisualMissile | 0 |

Le rapport indique zéro référence non résolue parmi les références non nulles effectivement sélectionnées et vérifiées. Ce chiffre ne signifie pas que chaque sort possède une chaîne visuelle complète : neuf sorts n'ont simplement aucune entrée SpellXSpellVisual, ce qui est traité comme une absence optionnelle.

## Relations parent → sort déclenché

Toutes les relations observées sont à la profondeur 0 du graphe fourni. Aucun des 17 sorts enfants n'a produit de nouvel enfant au niveau suivant.

| Talent parent | Parent SpellID | Effet | Sort déclenché | Child SpellID |
|---|---:|---:|---|---:|
| Power Strikes | 121817 | 0 | Power Strikes | 129914 |
| Rushing Jade Wind | 116847 | 0 | Rushing Jade Wind | 148187 |
| Breath of Sindragosa | 152279 | 0 | Breath of Sindragosa | 155166 |
| Refreshing Jade Wind | 196725 | 0 | Refreshing Jade Wind | 162530 |
| Frozen Pulse | 194909 | 0 | Frozen Pulse | 195750 |
| Blighted Rune Weapon | 194918 | 0 | Blighted Rune Weapon | 195758 |
| Bloodworms | 195679 | 0 | Bloodworm | 196361 |
| Eye of the Tiger | 196607 | 0 | Eye of the Tiger | 196608 |
| Special Delivery | 196730 | 0 | Special Delivery | 196734 |
| Prepared | 203551 | 0 | Prepared | 203650 |
| Demon Blades | 203555 | 0 | Demon Blades | 203796 |
| Desperate Instincts | 205411 | 0 | Desperate Instincts | 205478 |
| Avalanche | 207142 | 0 | Avalanche | 207150 |
| Chaos Blades | 247938 | 0 | Chaos Blades | 211797 |
| Master of the Glaive | 203556 | 0 | Master of the Glaive | 213405 |
| Runic Attenuation | 207104 | 0 | Runic Attenuation | 221322 |
| Blade Turning | 247254 | 0 | Blade Turning | 247253 |

Ces relations sont des références SpellEffect.TriggerSpell telles qu'extraites. Elles ne prouvent pas à elles seules que le sort enfant est une capacité lancée directement par le joueur : il peut être une aura, un proc ou un effet interne.

## Sorts enfants avec une entrée visuelle

| SpellID | Nom | SpellVisualID | SpellVisualKitID observés | Modèles FileDataID présents dans CASC |
|---:|---|---:|---|---|
| 148187 | Rushing Jade Wind | 33982 | 36828 | 573524 |
| 155166 | Breath of Sindragosa | 36634 | 40635, 40636 | 948165, 948167 |
| 162530 | Refreshing Jade Wind | 40001 | 45129 | 627364 |
| 195750 | Frozen Pulse | 50802 | 52516, 67720 | 166211, 166385, 999067 |
| 203796 | Demon Blades | 53283 | 62457, 64353 | 1290883 |
| 207150 | Avalanche | 62663 | 77651 | 1525789 |
| 211797 | Chaos Blades | 55771 | 67709, 68047 | aucun modèle attaché dans les lignes émises |
| 213405 | Master of the Glaive | 56401 | 360 | 166898 |

Les modèles listés ont été vérifiés par FileDataID au moyen de CASC. Les chemins inverses ne sont pas disponibles dans le rapport. Aucun chemin de modèle ou texture ne doit donc être inventé à partir de ces identifiants.

Les neuf sorts sans entrée SpellXSpellVisual dans cet export sont : 129914 Power Strikes, 195758 Blighted Rune Weapon, 196361 Bloodworm, 196608 Eye of the Tiger, 196734 Special Delivery, 203650 Prepared, 205478 Desperate Instincts, 221322 Runic Attenuation et 247253 Blade Turning.

## Ce que les rapports permettent — et ne permettent pas — de conclure

### Conclusions supportées

- Huit sorts enfants ont une entrée SpellXSpellVisual et une SpellVisual correspondante.
- Des kits, effets de kits et attachments de modèles sont liés à ces huit sorts.
- Les FileDataID de modèles non nuls émis dans le rapport ont été trouvés dans le CASC local.
- Aucun missile n'a été émis dans la table SpellVisualMissile pour ces chaînes.
- Aucun sort déclenché secondaire n'a été trouvé à la profondeur 1.
- Les chemins inverses modèle/texture sont indisponibles.

### Conclusions non supportées

Les valeurs numériques brutes ne sont pas traduites en sémantique d'animation. En particulier :

- les valeurs brutes StartEvent / EndEvent ne sont pas interprétées comme des enums ;
- les rôles des attachments ne sont pas déduits de leur seul numéro ;
- les SoundKit, motion, positioner, animation-kit et targeting-kit IDs ne sont pas résolus vers leurs tables ;
- un FileDataID présent dans CASC ne garantit pas que le renderer de Survivors puisse charger ou jouer cet asset ;
- un kit visuel associé à un sort ne garantit pas qu'il s'agit de l'animation de cast principale.

Les champs Animation IDs émis dans les model attachments sont à -1 ou 0 dans les lignes observées ; aucune animation jouable n'est donc établie par ces champs.

## Recommandations pour l'adaptation Survivors

1. Ne pas créer automatiquement une animation depuis un SpellVisualKitID.
2. Séparer les données de présentation (SpellVisual, kit, modèle) du comportement de gameplay (AbilityDef / Kind).
3. Pour les huit sorts mappés, procéder à une inspection visuelle des modèles et effets avec un outil local compatible avec les formats Legion, sans copier les assets dans Git.
4. Pour les neuf sorts sans SpellXSpellVisual, examiner les SpellEffect et les auras du parent et de l'enfant ; ils peuvent ne pas nécessiter de visual dédié.
5. Traiter séparément les références SoundKit, motion, positioner et animation kit : les tables cibles n'étaient pas dans le périmètre de cette extraction.
6. N'intégrer dans Survivors que les assets dont le chemin, le rôle et la compatibilité avec le renderer sont confirmés.

## Reproductibilité

Le mainteneur rapporte que les fichiers ont été générés avec les définitions locales en lecture seule sur le build 7.3.5.26972. Les hashes des sorties sont :

| Fichier | SHA-256 |
|---|---|
| talent-spell-details.csv | `43906368a168ac74f5988f249acc571d6177e295382c6c42b7528b67d364aa3d` |
| trigger-spell-details.csv | `1fd0f6a54389f644978971ec9d40efc1090aa7f8ab6862288a70bf9dd7a4d0d2` |
| trigger-visual-chain.csv | `f36a0ba944c79b9b4617f45ff02f9faea2b260ad26ebc6c1cc544bcdaeddfb89` |
| trigger-unresolved-references.csv | `91f9320785388711145ba925725fd7b0f1f8dee0680735cad9c841df2b743ad9` |

Les CSV eux-mêmes restent locaux et ne sont pas inclus dans Git.
