# Toc Toc

Un petit programme pour se faire signe entre un PC Windows et un Mac : un clic, et l'autre
ordinateur affiche une notification qui se secoue et joue un petit son.

## Installer

**Sur le PC (Windows)**
Double-cliquez sur `Toc Toc_0.1.0_x64-setup.exe`, puis « Suivant » jusqu'au bout.
Windows peut afficher un écran bleu « Windows a protégé votre ordinateur » : cliquez sur
**Informations complémentaires**, puis **Exécuter quand même**. (C'est normal : le programme
n'a pas de signature payante.)

**Sur le Mac**
Double-cliquez sur le fichier `.dmg`, puis glissez **Toc Toc** dans **Applications**.
Au premier lancement, macOS refuse d'ouvrir une application qui ne vient pas de l'App Store :
ouvrez **Réglages Système → Confidentialité et sécurité**, descendez, puis cliquez sur
**Ouvrir quand même** à côté de « Toc Toc ». C'est à faire une seule fois.

> ⚠️ La version Mac n'a pas encore été essayée sur un vrai Mac. Elle se fabrique
> automatiquement sur GitHub (voir « Fabriquer les installateurs » plus bas).

## Associer les deux ordinateurs (une seule fois)

1. Ouvrez Toc Toc sur les deux ordinateurs.
2. Sur l'un : **Créer un code**. Un code de 8 caractères s'affiche.
3. Sur l'autre : **J'ai déjà un code**, tapez-le, puis **Associer**.

C'est tout. Pas de compte, pas de réglage de box ni de pare-feu, et cela fonctionne même si
les deux ordinateurs ne sont pas sur le même réseau. Le code est valable 10 minutes et ne
sert qu'une fois.

## Envoyer un ping

- **Un clic** sur l'icône Toc Toc (en bas à droite sur Windows, près de l'horloge ; en haut à
  droite sur le Mac, dans la barre de menus), ou
- le raccourci clavier **Ctrl + Alt + P** (Windows) / **⌘ ⌥ P** (Mac), ou
- la fenêtre de Toc Toc, pour ajouter un petit mot.

Un **clic droit** sur l'icône ouvre le menu : écrire un message, Ne pas déranger, Réglages,
Quitter. Fermer la fenêtre ne quitte pas Toc Toc : il reste près de l'horloge.

Sur Windows, l'icône peut être rangée derrière la petite flèche **^** près de l'horloge :
faites-la glisser sur la barre des tâches pour la garder visible.

## Réglages (propres à chaque ordinateur)

Son (cinq sons fournis, ou votre propre fichier wav / mp3 / ogg / flac), volume, position de
la notification à l'écran, taille, durée d'affichage, lancement au démarrage, raccourci
clavier et **Ne pas déranger**. Chaque changement est enregistré tout de suite.

Un Mac ne peut pas vibrer : la « vibration » est la secousse de la notification à l'écran.

## Vie privée

- Les messages passent par un relais public gratuit (ntfy.sh). Ils sont **chiffrés de bout en
  bout** : le relais ne voit que du texte illisible, sur un canal au nom impossible à deviner.
- Aucun nom, aucune adresse e-mail, aucun compte n'est envoyé nulle part.
- La clé reste dans un fichier de réglages sur chaque ordinateur. « Dissocier cet ordinateur »
  l'efface sur les deux.

## Bon à savoir

- Il faut Internet sur les deux ordinateurs, et Toc Toc ouvert des deux côtés.
- Si l'autre ordinateur était éteint ou en veille, il reçoit à son réveil les pings des
  10 dernières minutes ; les plus anciens sont perdus.
- Après l'envoi, une petite pastille confirme « Ping envoyé », puis « Reçu » quand l'autre
  ordinateur l'a bien affiché.
- Le relais gratuit est un service tiers : s'il est en panne, les pings ne passent pas.

## Fabriquer les installateurs (pour la personne qui s'occupe de la technique)

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

Essayer deux ordinateurs sur un seul PC : lancer une seconde copie avec la variable
`TOCTOC_PROFILE=b` (réglages séparés).
