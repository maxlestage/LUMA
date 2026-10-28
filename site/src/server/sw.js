// Le service worker de LÜMA : le site s'ouvre hors ligne, installé comme une app.
//
// - les pages : le réseau d'abord (toujours à jour), la copie sinon, et l'accueil
//   hors ligne en dernier recours ;
// - les fichiers du site (styles, scripts, 3D, WebAssembly, images) : la copie tout de
//   suite, rafraîchie en arrière-plan ;
// - les polices de Google : laissées au navigateur (la politique de sécurité du
//   service worker n'autorise que le site lui-même) ;
// - le téléchargement de l'app : jamais mis en cache (trop gros, toujours le dernier) ;
// - les réponses `no-store` (la page de la licence : la clé, l'adresse) : jamais gardées.
const CACHE = "luma-__VERSION__";
const PREFIX = "luma-";
const PRECACHE = __PRECACHE__;
const OFFLINE = "/";

self.addEventListener("install", (event) => {
  event.waitUntil(
    caches.open(CACHE)
      .then((cache) => cache.addAll(PRECACHE))
      .then(() => self.skipWaiting()),
  );
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    caches.keys()
      .then((keys) => Promise.all(
        keys.filter((key) => key.startsWith(PREFIX) && key !== CACHE).map((key) => caches.delete(key)),
      ))
      .then(() => self.clients.claim()),
  );
});

function keep(request, response) {
  const store = response && !/no-store/i.test(response.headers.get("cache-control") || "");
  if (store && response.ok) {
    const copy = response.clone();
    caches.open(CACHE).then((cache) => cache.put(request, copy));
  }
  return response;
}

self.addEventListener("fetch", (event) => {
  const request = event.request;
  if (request.method !== "GET") return;
  const url = new URL(request.url);
  if (url.origin !== self.location.origin) return;
  if (url.pathname.includes("/download/")) return;

  if (request.mode === "navigate") {
    event.respondWith(
      fetch(request)
        .then((response) => keep(request, response))
        .catch(() => caches.match(request, { ignoreSearch: true })
          .then((cached) => cached || caches.match(OFFLINE))),
    );
    return;
  }
  event.respondWith(
    caches.match(request).then((cached) => {
      const fresh = fetch(request)
        .then((response) => keep(request, response))
        .catch(() => cached || Response.error());
      return cached || fresh;
    }),
  );
});
