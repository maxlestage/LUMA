# LÜMA Premium : abonnements et téléchargement

LÜMA se télécharge librement et s'essaie **14 jours**, tout compris. Ensuite, il faut
un abonnement **LÜMA Premium** :

| Formule | Prix TTC | |
|---|---|---|
| Mensuel | 9,99 € par mois | sans engagement |
| Annuel | 79,99 € par an | deux mois offerts (6,67 € par mois) |

Les prix affichés sont dans `crates/luma-licence/src/lib.rs` (l'app) et
`site/src/content.rs` (le site) ; les prix facturés sont ceux de Stripe.

## Comment ça marche

- **Paiement : Stripe**, par deux liens de paiement (Payment Links), un par formule.
  Stripe encaisse, renouvelle, envoie les factures et gère le portail client
  (moyen de paiement, factures, résiliation). Stripe n'est **pas** revendeur officiel :
  la TVA est à votre charge (Stripe Tax la calcule et l'applique, voir plus bas).
- **La clé de licence** : après le paiement, Stripe renvoie le client sur
  `https://SITE/licence/?session_id=…`. Le serveur du site demande à Stripe la session
  et l'abonnement, puis **signe** (Ed25519) une clé `LUMA1.…` qui contient le courriel,
  la formule et sa date de fin (fin de la période payée + 14 jours). La page l'affiche ;
  la même adresse la redonne à tout moment. Pas de base de données : tout est dans la clé.
- **Dans l'app** (`luma-licence`, `apps/studio/src/licence.rs`) : au premier lancement,
  l'essai commence. La puce de la barre du haut montre « Essai · 12 j » ou « Premium » ;
  un clic ouvre l'écran Premium (formules, liens de paiement, champ de la clé). La clé
  est vérifiée **hors ligne**, avec la clé publique compilée dans l'app. À la fin de
  l'essai, l'écran Premium recouvre l'app jusqu'à l'activation d'une clé.
- **Renouvellement** : une fois par jour, en ligne, l'app envoie sa clé à
  `POST /api/licence` ; le serveur vérifie la signature, redemande l'abonnement à Stripe
  et rend une clé prolongée. Un abonnement résilié ou impayé n'est plus prolongé : l'app
  se reverrouille à la date de fin de sa clé. Le réseau passe par `curl` (aucune
  bibliothèque TLS compilée dans LÜMA).
- **Limite** : sans base de données, le nombre d'appareils par clé n'est pas limité ; la
  clé porte le courriel de l'abonné, ce qui décourage le partage.
- **iPhone et iPad** : l'App Store impose ses achats intégrés pour un abonnement vendu
  dans une app ; ce sera fait à la publication sur l'App Store (compte Apple Developer).
  D'ici là, une clé y est vérifiée hors ligne et vaut jusqu'à son terme.
- La ligne de commande `luma` reste libre.

## Ouvrir la boutique (une fois)

1. **Les clés de signature** : `cargo run -p luma-cli -- licence-keys` affiche
   `LUMA_LICENCE_SECRET=…` (privée : seulement sur Heroku, jamais dans le dépôt) et
   `LUMA_LICENCE_PUBLIC=…` (publique : compilée dans l'app).
2. **Stripe** (<https://dashboard.stripe.com>) : créer un produit **LÜMA Premium** avec
   deux prix récurrents, **9,99 € par mois** et **79,99 € par an** (TTC : « taxe
   incluse »). Pour chacun, créer un **lien de paiement** ; dans « Après le paiement »,
   choisir « Ne pas afficher la page de confirmation, rediriger vers votre site » :
   `https://SITE/licence/?session_id={CHECKOUT_SESSION_ID}`.
3. **TVA** : activer **Stripe Tax** (Paramètres → Taxes), y déclarer votre
   immatriculation, et cocher « Collecter automatiquement les taxes » sur les liens.
   Au‑delà de 10 000 € de ventes par an dans l'UE hors de France, le guichet OSS
   s'applique.
4. **Portail client** : Paramètres → Facturation → Portail client : l'activer
   (résiliation, factures, moyen de paiement) et copier son **lien de connexion**.
5. **Une clé API restreinte** (Développeurs → Clés API → Créer une clé restreinte) :
   lecture seule sur *Checkout Sessions* et *Subscriptions*. Elle commence par `rk_live_`.
6. **Heroku** :
   `heroku config:set STRIPE_SECRET_KEY=rk_live_… LUMA_LICENCE_SECRET=… LUMA_CHECKOUT_MONTHLY=https://buy.stripe.com/… LUMA_CHECKOUT_ANNUAL=https://buy.stripe.com/… LUMA_BILLING_PORTAL=https://billing.stripe.com/p/login/…`
   (effet immédiat sur le site : boutons, page `/licence/`, `/abonnement/`, `/api/licence`).
7. **GitHub** : **Settings → Secrets and variables → Actions → Variables**, ajouter
   `LUMA_LICENCE_PUBLIC`, `LUMA_LICENCE_SERVER` (`https://SITE`), `LUMA_BILLING_PORTAL`,
   `LUMA_CHECKOUT_MONTHLY` et `LUMA_CHECKOUT_ANNUAL`. Puis créer une nouvelle version
   (Actions → Release) : les boutons « S'abonner » de l'app s'activent. Sans ces
   variables, ils affichent « Bientôt disponible ».

Pour essayer sans rien encaisser : les mêmes étapes en **mode test** de Stripe
(clé `rk_test_…`, carte `4242 4242 4242 4242`).

## Télécharger

- **Site** : le bouton « Télécharger pour Mac » de la section Tarifs ; au déploiement
  sur Heroku, `heroku-build.sh` récupère l'app de la dernière Release (même privée).
- **Releases GitHub** : Actions → Release → Run workflow (version `0.1.0`…), ou une étiquette `v…` (`git tag v0.1.0 && git push origin
  v0.1.0`) construit et publie l'app Mac universelle, l'app iPhone et iPad non signée,
  et la ligne de commande pour Mac et Linux (`.github/workflows/release.yml`).
- **Signature et notarisation** : sans compte Apple Developer, l'app est signée
  « ad hoc » et macOS 15 refuse de l'ouvrir au premier lancement (Réglages Système →
  Confidentialité et sécurité → « Ouvrir quand même »). Pour la vendre, il faut qu'elle
  s'ouvre d'un double clic : signature **Developer ID** et **notarisation** par Apple,
  faites par la CI dès que les secrets sont là :
  1. S'inscrire à l'Apple Developer Program (99 € par an) : <https://developer.apple.com/programs/>.
  2. Sur un Mac, Xcode → Réglages → Comptes → Gérer les certificats → « + » →
     **Developer ID Application** ; puis Trousseau d'accès → ce certificat → Exporter
     en `.p12`, avec un mot de passe.
  3. App Store Connect → Utilisateurs et accès → Intégrations → **Clés d'API** :
     créer une clé (accès « Développeur »), télécharger le `.p8`, noter son
     identifiant (Key ID) et l'Issuer ID.
  4. GitHub → Settings → Secrets and variables → Actions → **Secrets** :
     - `MACOS_CERTIFICATE` : `base64 -i certificat.p12 | pbcopy`, puis coller ;
     - `MACOS_CERTIFICATE_PASSWORD` : le mot de passe du `.p12` ;
     - `APPLE_API_KEY` : le contenu du `.p8` ;
     - `APPLE_API_KEY_ID` et `APPLE_API_ISSUER`.
  5. Actions → Release → Run workflow : l'app est signée, notariée, agrafée ; la note
     « Ouvrir quand même » disparaît du site (sur Heroku : `LUMA_MAC_NOTARIZED=1`).