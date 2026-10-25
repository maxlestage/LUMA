# LÜMA Premium : abonnements et téléchargement

LÜMA se télécharge librement et s'essaie **14 jours**, tout compris. Ensuite, il faut
un abonnement **LÜMA Premium** :

| Formule | Prix TTC | |
|---|---|---|
| Mensuel | 9,99 € par mois | sans engagement |
| Annuel | 79,99 € par an | deux mois offerts (6,67 € par mois) |

Les prix affichés sont dans `crates/luma-licence/src/lib.rs` (l'app) et
`site/src/content.rs` (le site) ; les prix facturés sont ceux de la boutique.

## Comment ça marche

- **Paiement : Lemon Squeezy**, vendeur officiel (« merchant of record ») : il encaisse,
  déclare et reverse la TVA de chaque pays, gère les renouvellements, les résiliations,
  les reçus, et envoie à chaque client une **clé de licence** par courriel.
- **Dans l'app** (`luma-licence`, `apps/studio/src/licence.rs`) : au premier lancement,
  l'essai commence. La puce de la barre du haut montre « Essai · 12 j » ou « Premium » ;
  un clic ouvre l'écran Premium (formules, lien de paiement, champ de la clé). À la fin
  de l'essai, l'écran Premium recouvre l'app jusqu'à l'activation d'une clé.
- **Vérification** : la clé est activée sur la machine (`/v1/licenses/activate`), puis
  revérifiée chaque jour (`/v1/licenses/validate`). Hors ligne, LÜMA reste ouvert
  14 jours après la dernière vérification réussie. Un abonnement résilié ou impayé :
  Lemon Squeezy désactive la clé, l'app se reverrouille à la vérification suivante.
  Le réseau passe par `curl` (aucune bibliothèque TLS compilée dans LÜMA).
- **iPhone et iPad** : l'App Store impose ses achats intégrés pour un abonnement vendu
  dans une app ; ce sera fait à la publication sur l'App Store (compte Apple Developer).
  D'ici là, une clé y est acceptée sans vérification en ligne.
- La ligne de commande `luma` reste libre.

## Ouvrir la boutique (une fois)

1. Créer un compte sur <https://www.lemonsqueezy.com>, puis une boutique.
2. Créer un produit **LÜMA Premium**, de type abonnement, avec deux variantes :
   **Mensuel** (9,99 €, tous les mois) et **Annuel** (79,99 €, tous les ans). Dans
   chaque variante, cocher **Generate license keys** (limite d'activations : 3, par
   exemple ; durée : celle de l'abonnement).
3. Copier les deux liens de paiement (« Share » → lien de checkout) et l'identifiant
   de la boutique.
4. Dans le dépôt GitHub : **Settings → Secrets and variables → Actions → Variables**,
   ajouter :
   - `LUMA_CHECKOUT_MONTHLY` : le lien de la variante Mensuel ;
   - `LUMA_CHECKOUT_ANNUAL` : le lien de la variante Annuel ;
   - `LUMA_STORE_ID` : l'identifiant de la boutique (seules ses clés seront acceptées).
5. Pour le site sur Heroku : `heroku config:set LUMA_CHECKOUT_MONTHLY=… LUMA_CHECKOUT_ANNUAL=…`
   (effet immédiat). Pour l'app : créer une nouvelle version (Actions → Release) ; ses
   boutons « S'abonner » s'activent. Sans ces variables, ils affichent « Bientôt
   disponible ».

## Télécharger

- **Site** : le bouton « Télécharger pour Mac » de la section Tarifs ; au déploiement
  sur Heroku, `heroku-build.sh` récupère l'app de la dernière Release (même privée).
- **Releases GitHub** : Actions → Release → Run workflow (version `0.1.0`…), ou une étiquette `v…` (`git tag v0.1.0 && git push origin
  v0.1.0`) construit et publie l'app Mac universelle, l'app iPhone et iPad non signée,
  et la ligne de commande pour Mac et Linux (`.github/workflows/release.yml`).
- **Signature** : l'app Mac est signée « ad hoc » : au premier lancement, clic droit →
  Ouvrir. Pour une ouverture directe (signature Developer ID et notarisation Apple), il
  faut un compte Apple Developer (99 $ par an).
