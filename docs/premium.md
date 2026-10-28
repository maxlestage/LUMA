# LÜMA Premium : abonnements et téléchargement

LÜMA se télécharge librement et s'essaie **un mois**, tout compris. Ensuite, il faut
un abonnement **LÜMA Premium** :

| Formule | Prix TTC | |
|---|---|---|
| Mensuel | 19,99 € par mois | sans engagement |
| Annuel | 239,88 € par an | soit 19,99 € par mois, en un seul paiement |

Ces abonnements sont ceux du Mac : **sur Mac, LÜMA Premium s'achète uniquement par
Stripe** (sur le site ; LÜMA n'est pas vendu sur le Mac App Store). **Sur iPhone et iPad
(iOS et iPadOS), il s'achète uniquement dans l'App Store** (abonnement intégré, distinct,
facturé par Apple), comme l'exigent les règles de l'App Store pour du contenu numérique.

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
- **iPhone et iPad** : un mois d'essai, puis un **achat unique à vie de 89,99 €**,
  vendu uniquement par l'App Store en achat intégré non renouvelable (StoreKit, appelé
  depuis Rust : `luma_apple::store`). L'écran Premium y montre cet achat à son prix App
  Store (dans la devise de l'acheteur), « Acheter » et « Restaurer les achats ». L'achat
  (ou sa restauration) est enregistré dans `licence.json` et n'expire jamais
  (`Plan::Lifetime`). Le partage familial est activé. Les clés de licence n'y comptent
  pas (et l'app Mac ignore l'App Store), sauf les clés offertes.
- **Accès offert** (`LUMA_OFFERTS`, sur Heroku : `courriel:code`, séparés par des
  virgules) : sur ordinateur, la personne tape son courriel et son code sur
  `/licence/offert/` et reçoit sa clé. Sur iPhone et iPad, elle touche **« Se connecter
  avec Apple »** dans l'écran Premium (`luma_apple::signin`) : l'app envoie le jeton
  d'identité d'Apple à `POST /api/licence/apple`, le serveur vérifie sa signature
  (clés publiques d'Apple), son destinataire (`app.luma.studio`) et le courriel
  vérifié, puis rend une clé offerte si ce courriel est dans la liste (le code n'est
  pas demandé : Apple a prouvé le courriel). Une adresse masquée (relais d'Apple) n'est
  pas acceptée. La clé se renouvelle chaque jour comme les autres ; retiré de la liste,
  l'accès s'arrête au plus tard 44 jours après.
- La ligne de commande `luma` reste libre.

## Ouvrir l'App Store (iPhone et iPad, une fois)

1. App Store Connect → l'app LÜMA (identifiant `app.luma.studio`) → **Achats
   intégrés** : « LÜMA Premium à vie », non consommable, identifiant
   `app.luma.studio.premium.lifetime`, 89,99 € (territoire de référence : France), en
   vente dans tous les pays, nom et description en français, anglais et espagnol. Créé
   par l'API App Store Connect le 27 octobre 2026, avec sa capture pour la
   vérification le 28 octobre : « Prêt à soumettre », à envoyer avec la version 1.0.

   Un autre identifiant ? Le donner à la compilation : variable `LUMA_APPSTORE_LIFETIME`.
2. Accords, taxes et coordonnées bancaires : l'accord « Applications payantes » signé.
3. « Se connecter avec Apple » : la capacité est activée sur l'identifiant
   `app.luma.studio` (`scripts/asc.py bundle-id` s'en assure à chaque envoi), et le
   profil en porte le droit `com.apple.developer.applesignin`.
4. Pour essayer sans payer : un compte **Sandbox** (Utilisateurs et accès → Sandbox),
   puis l'app installée par TestFlight sur un iPhone ou un iPad.

## Ouvrir la boutique (une fois)

1. **Les clés de signature** : `cargo run -p luma-cli -- licence-keys` affiche
   `LUMA_LICENCE_SECRET=…` (privée : seulement sur Heroku, jamais dans le dépôt) et
   `LUMA_LICENCE_PUBLIC=…` (publique : compilée dans l'app).
2. **Stripe** (<https://dashboard.stripe.com>) : créer un produit **LÜMA Premium** avec
   deux prix récurrents, **19,99 € par mois** et **239,88 € par an** (TTC : « taxe
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