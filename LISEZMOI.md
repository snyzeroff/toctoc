# Toc Toc

Un petit programme pour se faire signe entre un PC Windows et un Mac : un clic, et l'autre
ordinateur affiche une notification qui se secoue et joue un petit son.

## Installer

**Sur le PC (Windows)**
Double-cliquez sur `Toc Toc_0.3.0_x64-setup.exe`, puis « Suivant » jusqu'au bout.
Windows peut afficher un écran bleu « Windows a protégé votre ordinateur » : cliquez sur
**Informations complémentaires**, puis **Exécuter quand même**. (C'est normal : le programme
n'a pas de signature payante.)

**Sur le Mac**
Double-cliquez sur le fichier `.dmg`, puis glissez **Toc Toc** dans **Applications**.
Au premier lancement, macOS refuse d'ouvrir une application qui ne vient pas de l'App Store :
ouvrez **Réglages Système → Confidentialité et sécurité**, descendez, puis cliquez sur
**Ouvrir quand même** à côté de « Toc Toc ». C'est à faire une seule fois.

**Si le Mac dit que Toc Toc « est endommagé » ou « ne peut pas être ouvert »** (fréquent pour une
application téléchargée sans signature Apple payante) : après l'avoir glissé dans Applications,
ouvrez le **Terminal** (Spotlight → « Terminal ») et collez cette ligne, puis Entrée :

```bash
xattr -cr "/Applications/Toc Toc.app"
```

Puis rouvrez Toc Toc normalement.

**Plan B : fabriquer Toc Toc directement sur le Mac** (aucun blocage possible, 5 à 10 minutes la
première fois). Récupérez le projet (branche `toctoc` du dépôt GitHub), puis dans le Terminal :

```bash
git clone -b toctoc https://github.com/barendrechtmats-lgtm/Inkbook.git toctoc
```

```bash
bash toctoc/scripts/installer-mac.sh
```

Le script vérifie les outils nécessaires (il propose d'installer ce qui manque), fabrique Toc Toc,
le met dans Applications et l'ouvre.

> ⚠️ La version Mac n'a pas encore été essayée sur un vrai Mac. Si quelque chose bloque,
> notez le message exact affiché (ou faites une capture d'écran).

## Associer les deux ordinateurs (une seule fois)

1. Ouvrez Toc Toc sur les deux ordinateurs.
2. Sur l'un : **Créer un code**. Un code de 8 caractères s'affiche.
3. Sur l'autre : **J'ai déjà un code**, tapez-le, puis **Associer**.

C'est tout. Pas de compte, pas de réglage de box ni de pare-feu, et cela fonctionne même si
les deux ordinateurs ne sont pas sur le même réseau. Le code est valable 10 minutes et ne
sert qu'une fois.

## Utiliser Toc Toc

**La conversation** (fenêtre principale) garde tout l'historique, sur chaque ordinateur.
On y écrit, on peut **mettre en forme** le texte (boutons en haut de la zone d'écriture, ou
`**gras**`, `*italique*`, listes avec `- `, code entre accents graves, liens), joindre une **image** (bouton image,
Ctrl + V pour coller, ou glisser-déposer) et **dessiner** (bouton crayon : couleurs, épaisseurs,
gomme, annuler). Cliquez sur une image reçue pour l'agrandir, puis **Dessiner dessus** pour
l'annoter et la renvoyer. Entrée envoie ; Maj + Entrée va à la ligne.

**La fenêtre rapide** s'ouvre au milieu de l'écran avec un raccourci, pour répondre vite sans
quitter ce qu'on fait. Elle montre les derniers messages ; Échap ou un clic ailleurs la range.
Le bouton **⤢** l'agrandit en conversation complète.

**Les raccourcis** (modifiables dans les Réglages : cliquez dessus puis appuyez sur les touches) :

| Action | Windows | Mac |
|---|---|---|
| Fenêtre rapide | Ctrl + Alt + M | ⌘ ⌥ M |
| Ping immédiat | Ctrl + Alt + P | ⌘ ⌥ P |
| Ouvrir la conversation | Ctrl + Alt + O | ⌘ ⌥ O |

Si un raccourci est déjà pris par une autre application, Toc Toc l'affiche en rouge : choisissez-en
un autre.

**L'icône** près de l'horloge (barre des tâches sur Windows, barre de menus en haut sur Mac) :
un clic envoie un ping ; un clic droit ouvre le menu (message rapide, conversation, Ne pas
déranger, Réglages, Quitter). Fermer la fenêtre ne quitte pas Toc Toc.
Sur Windows, l'icône peut être rangée derrière la petite flèche **^** près de l'horloge :
faites-la glisser sur la barre des tâches pour la garder visible.

**Les notifications** : un message ou un ping qui arrive s'affiche en petit avec un son (et une
légère secousse pour un ping). Un clic dessus ouvre la fenêtre rapide pour répondre. Si la
conversation est déjà ouverte devant vous, seul le son est joué.

**Copier un message** : passez la souris sur un message (le vôtre ou celui de l'autre), un bouton
**Copier** apparaît dessous. Il copie le texte, ou l'image si le message n'est qu'une image.

## Mises à jour automatiques

À partir de la version 0.3.0, Toc Toc cherche tout seul une nouvelle version au démarrage puis
toutes les 6 heures, la télécharge en arrière-plan, puis affiche un bandeau « Toc Toc x.y est prêt
à être installé » avec un bouton **Redémarrer**. Un clic, et c'est à jour. On peut aussi vérifier
dans **Réglages → Mises à jour → Vérifier**.

La version 0.3.0 doit être installée une dernière fois à la main sur chaque ordinateur ; ensuite,
plus besoin.

## Réglages (propres à chaque ordinateur)

Votre prénom (affiché chez l'autre), son (cinq sons fournis ou votre propre fichier), volume,
position, taille et durée de la notification, raccourcis, Ne pas déranger, lancement au démarrage,
et **Effacer l'historique** (sur cet ordinateur seulement). La roue dentée en haut à droite de la
conversation ouvre les Réglages.

Un Mac ne peut pas vibrer : la « vibration » est la secousse de la notification à l'écran.

## Vie privée

- Les messages et les images passent par un relais public gratuit (ntfy.sh). Ils sont **chiffrés
  de bout en bout** : le relais ne voit que des données illisibles, sur un canal au nom
  impossible à deviner. Les gros messages (images) y restent au plus 3 heures, toujours chiffrés.
- L'historique est enregistré **en clair sur chaque ordinateur** (dossier de réglages de Toc Toc),
  comme dans la plupart des messageries.
- Aucun nom, aucune adresse e-mail, aucun compte n'est envoyé nulle part.
- La clé reste dans un fichier de réglages sur chaque ordinateur. « Dissocier cet ordinateur »
  l'efface sur les deux.

## Bon à savoir

- Il faut Internet sur les deux ordinateurs, et Toc Toc ouvert des deux côtés.
- Si l'autre ordinateur était éteint ou en veille, il reçoit à son réveil les messages des
  10 dernières minutes ; les plus anciens sont perdus.
- Sous chaque message envoyé : « ✓ » quand il est parti, « ✓✓ reçu » quand l'autre ordinateur
  l'a reçu. S'il n'a pas pu partir, un bouton **Réessayer** apparaît.
- Les deux ordinateurs doivent avoir la même version de Toc Toc (0.2 ou plus) pour les
  messages, images et dessins ; les pings passent aussi avec la 0.1.
- Images : 8 Mo au plus ; les grandes photos sont réduites automatiquement.
- Le relais gratuit est un service tiers : s'il est en panne, les pings ne passent pas.

## Publier une nouvelle version (pour la personne qui s'occupe de la technique)

1. Changer le numéro de version dans `package.json`, `src-tauri/tauri.conf.json` et
   `src-tauri/Cargo.toml` (par exemple 0.3.1).
2. Enregistrer et envoyer, puis poser l'étiquette de version :

```powershell
git tag v0.3.1
```

```powershell
git push origin main v0.3.1
```

GitHub fabrique alors Mac et Windows (environ 15 minutes) et publie la version ; les Toc Toc
installés la proposent ensuite d'eux-mêmes. Cela demande que le dépôt soit **public** (pour que
les applications puissent télécharger la mise à jour) et que la clé de signature soit enregistrée
dans les secrets du dépôt sous le nom `TAURI_SIGNING_PRIVATE_KEY`. La clé se trouve sur le PC dans
`%USERPROFILE%\.tauri\toctoc-updater.key` : **gardez-en une copie**, sans elle on ne peut plus
publier de mise à jour pour les applications déjà installées.

## Fabriquer les installateurs à la main

Windows, sur ce PC (Node et Rust installés) :

```powershell
npm install
```

```powershell
npx tauri build --bundles nsis
```

L'installateur est écrit dans `src-tauri\target\release\bundle\nsis\`.

Mac : il faut un Mac, ou GitHub. Le fichier `.github/workflows/installateurs.yml` fabrique le
`.dmg` (et l'installateur Windows) sur les machines de GitHub : une fois le projet poussé sur
un dépôt GitHub, onglet **Actions → Installateurs → Run workflow**, puis téléchargez
`TocToc-Mac`. Sur un Mac, la commande équivalente est
`npx tauri build --target universal-apple-darwin --bundles dmg`.

Essayer deux ordinateurs sur un seul PC : `scripts/essai.sh` lance deux copies avec des
réglages séparés (variable `TOCTOC_PROFILE`) ; `scripts/cdp.mjs` permet de piloter leur interface.
