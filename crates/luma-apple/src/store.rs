//! L'App Store sur iPhone et iPad : les abonnements de LÜMA Premium, par StoreKit.
//!
//! Sur iPhone et iPad, LÜMA Premium s'achète uniquement dans l'App Store (sur Mac,
//! uniquement par Stripe, sur le site). [`start`] inscrit un observateur des achats
//! auprès du système et demande les deux produits (leur prix, dans la devise de
//! l'acheteur) ; [`buy`] lance un achat, [`restore`] retrouve les achats faits sur un
//! autre appareil. Tout ce qui arrive revient par le canal rendu par [`start`], en
//! [`Event`] : à l'app de l'enregistrer (`luma_licence::record_store`).
//!
//! L'interface Objective‑C de StoreKit est appelée depuis Rust (comme ImageIO dans ce
//! crate) : aucun code Swift ni C à compiler. Apple la marque comme ancienne au profit
//! de StoreKit 2, réservé à Swift ; elle reste prise en charge et suffit à des
//! abonnements renouvelables.
//!
//! Ailleurs qu'iPhone et iPad, il n'y a pas d'App Store : [`start`] rend `None`.

use std::sync::mpsc::Receiver;

/// Un abonnement tel que l'App Store le vend : son identifiant et son prix, formaté
/// dans la devise et la langue de l'acheteur (« 19,99 € », « $19.99 »).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Product {
    pub id: String,
    pub price: String,
}

/// Ce que l'App Store répond.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// Les produits trouvés (ceux qui manquent ne sont pas encore en vente).
    Products(Vec<Product>),
    /// Un achat, un renouvellement ou une restauration : le produit et sa date
    /// (secondes Unix).
    Purchased { product: String, date: u64 },
    /// L'achat attend l'accord d'un parent (Partage familial).
    Deferred,
    /// L'acheteur a renoncé.
    Cancelled,
    /// L'achat a échoué : le message du système.
    Failed(String),
    /// La restauration est finie (les achats retrouvés sont arrivés avant).
    Restored,
}

/// Inscrit l'observateur des achats et demande les produits `ids`. À appeler une fois,
/// au démarrage, sur le fil principal. `None` sans App Store (ailleurs qu'iPhone et
/// iPad).
pub fn start(ids: &[&str]) -> Option<Receiver<Event>> {
    #[cfg(target_os = "ios")]
    {
        Some(ios::start(ids))
    }
    #[cfg(not(target_os = "ios"))]
    {
        let _ = ids;
        None
    }
}

/// Achète le produit `id` : le système affiche sa feuille de paiement.
pub fn buy(id: &str) {
    #[cfg(target_os = "ios")]
    ios::buy(id);
    #[cfg(not(target_os = "ios"))]
    let _ = id;
}

/// Retrouve les achats faits avec ce compte Apple (un autre appareil, une
/// réinstallation).
pub fn restore() {
    #[cfg(target_os = "ios")]
    ios::restore();
}

/// L'App Store est‑il là (iPhone et iPad) ?
pub const AVAILABLE: bool = cfg!(target_os = "ios");

#[cfg(target_os = "ios")]
#[allow(deprecated)] // StoreKit « 1 » : voir l'en‑tête du module.
mod ios {
    use std::cell::RefCell;
    use std::sync::mpsc::{Receiver, Sender, channel};

    use objc2::rc::Retained;
    use objc2::runtime::{NSObject, NSObjectProtocol, ProtocolObject};
    use objc2::{AllocAnyThread, DefinedClass, define_class, msg_send};
    use objc2_foundation::{NSArray, NSNumberFormatter, NSNumberFormatterStyle, NSSet, NSString};
    use objc2_store_kit::{
        SKErrorCode, SKMutablePayment, SKPaymentQueue, SKPaymentTransaction, SKPaymentTransactionObserver,
        SKPaymentTransactionState, SKProductsRequest, SKProductsRequestDelegate, SKProductsResponse, SKRequestDelegate,
    };

    use super::{Event, Product};

    struct Ivars {
        tx: Sender<Event>,
    }

    define_class!(
        // SAFETY: NSObject n'impose rien à ses sous‑classes, et `Observer` n'implémente
        // pas `Drop`.
        #[unsafe(super(NSObject))]
        #[name = "LumaStoreObserver"]
        #[ivars = Ivars]
        struct Observer;

        unsafe impl NSObjectProtocol for Observer {}

        // SAFETY: les signatures sont celles du protocole (objc2-store-kit).
        unsafe impl SKPaymentTransactionObserver for Observer {
            #[unsafe(method(paymentQueue:updatedTransactions:))]
            fn updated(&self, queue: &SKPaymentQueue, transactions: &NSArray<SKPaymentTransaction>) {
                for t in transactions {
                    // SAFETY: lectures de propriétés d'une transaction que le système
                    // vient de passer ; elle est terminée une seule fois, ici.
                    unsafe { self.transaction(queue, &t) };
                }
            }

            #[unsafe(method(paymentQueueRestoreCompletedTransactionsFinished:))]
            fn restored(&self, _queue: &SKPaymentQueue) {
                let _ = self.ivars().tx.send(Event::Restored);
            }

            #[unsafe(method(paymentQueue:restoreCompletedTransactionsFailedWithError:))]
            fn restore_failed(&self, _queue: &SKPaymentQueue, error: &objc2_foundation::NSError) {
                let _ = self
                    .ivars()
                    .tx
                    .send(Event::Failed(error.localizedDescription().to_string()));
            }
        }

        unsafe impl SKRequestDelegate for Observer {}

        // SAFETY: la signature est celle du protocole.
        unsafe impl SKProductsRequestDelegate for Observer {
            #[unsafe(method(productsRequest:didReceiveResponse:))]
            fn response(&self, _request: &SKProductsRequest, response: &SKProductsResponse) {
                // SAFETY: la réponse et ses produits sont des objets en lecture seule,
                // valables pendant cet appel ; le formateur est neuf et à nous.
                let products = unsafe {
                    let format = NSNumberFormatter::new();
                    format.setNumberStyle(NSNumberFormatterStyle::CurrencyStyle);
                    response
                        .products()
                        .iter()
                        .map(|p| {
                            format.setLocale(Some(&p.priceLocale()));
                            Product {
                                id: p.productIdentifier().to_string(),
                                price: format
                                    .stringFromNumber(&p.price())
                                    .map(|s| s.to_string())
                                    .unwrap_or_default(),
                            }
                        })
                        .collect()
                };
                let _ = self.ivars().tx.send(Event::Products(products));
            }
        }
    );

    impl Observer {
        fn new(tx: Sender<Event>) -> Retained<Self> {
            let this = Self::alloc().set_ivars(Ivars { tx });
            // SAFETY: `init` de NSObject, sur un objet tout juste alloué.
            unsafe { msg_send![super(this), init] }
        }

        /// Une transaction : l'achat (ou sa restauration) est annoncé, puis terminé ;
        /// un échec aussi ; une transaction en cours attend la suite.
        ///
        /// # Safety
        /// `t` vient du système, dans `paymentQueue:updatedTransactions:`.
        unsafe fn transaction(&self, queue: &SKPaymentQueue, t: &SKPaymentTransaction) {
            let tx = &self.ivars().tx;
            // SAFETY: voir l'appelant.
            unsafe {
                let state = t.transactionState();
                if state == SKPaymentTransactionState::Purchased || state == SKPaymentTransactionState::Restored {
                    let product = t.payment().productIdentifier().to_string();
                    let date = t.transactionDate().map_or(0.0, |d| d.timeIntervalSince1970()).max(0.0) as u64;
                    let _ = tx.send(Event::Purchased { product, date });
                    queue.finishTransaction(t);
                } else if state == SKPaymentTransactionState::Failed {
                    let event = match t.error() {
                        Some(e) if e.code() == SKErrorCode::PaymentCancelled.0 => Event::Cancelled,
                        Some(e) => Event::Failed(e.localizedDescription().to_string()),
                        None => Event::Cancelled,
                    };
                    let _ = tx.send(event);
                    queue.finishTransaction(t);
                } else if state == SKPaymentTransactionState::Deferred {
                    let _ = tx.send(Event::Deferred);
                }
            }
        }
    }

    thread_local! {
        // L'observateur et la demande de produits doivent vivre aussi longtemps que
        // l'app : le système ne les retient pas.
        static KEEP: RefCell<Option<(Retained<Observer>, Retained<SKProductsRequest>)>> =
            const { RefCell::new(None) };
    }

    pub fn start(ids: &[&str]) -> Receiver<Event> {
        let (tx, rx) = channel();
        let observer = Observer::new(tx);
        let names: Vec<Retained<NSString>> = ids.iter().map(|i| NSString::from_str(i)).collect();
        let set = NSSet::from_retained_slice(&names);
        // SAFETY: la file d'achats du système, un observateur et un délégué que nous
        // gardons en vie (KEEP) ; tout se passe sur le fil principal.
        let request = unsafe {
            SKPaymentQueue::defaultQueue().addTransactionObserver(ProtocolObject::from_ref(&*observer));
            let request = SKProductsRequest::initWithProductIdentifiers(SKProductsRequest::alloc(), &set);
            request.setDelegate(Some(ProtocolObject::from_ref(&*observer)));
            request.start();
            request
        };
        KEEP.with(|k| *k.borrow_mut() = Some((observer, request)));
        rx
    }

    pub fn buy(id: &str) {
        // SAFETY: un paiement neuf, remis à la file d'achats du système ; l'achat
        // revient à l'observateur.
        unsafe {
            if !SKPaymentQueue::canMakePayments() {
                return;
            }
            let payment = SKMutablePayment::new();
            payment.setProductIdentifier(&NSString::from_str(id));
            SKPaymentQueue::defaultQueue().addPayment(&payment);
        }
    }

    pub fn restore() {
        // SAFETY: la file d'achats du système ; les achats retrouvés reviennent à
        // l'observateur.
        unsafe { SKPaymentQueue::defaultQueue().restoreCompletedTransactions() };
    }
}
