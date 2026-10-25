// Le service worker de LÜMA : le site s'ouvre hors ligne, installé comme une app.
//
// - les pages : le réseau d'abord (toujours à jour), la copie sinon, et l'accueil
//   hors ligne en dernier recours ;
// - les fichiers du site (styles, scripts, 3D, WebAssembly, images) : la copie tout de
//   suite, rafraîchie en arrière-plan ;
// - les polices de Google : gardées après la première visite ;
// - le téléchargement de l'app : jamais mis en cache (trop gros, toujours le dernier).
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
  if (response && (response.ok || response.type === "opaque")) {
    const copy = response.clone();
    caches.open(CACHE).then((cache) => cache.put(request, copy));
  }
  return response;
}

self.addEventListener("fetch", (event) => {
  const request = event.request;
  if (request.method !== "GET") return;
  const url = new URL(request.url);
  const fonts = url.hostname === "fonts.googleapis.com" || url.hostname === "fonts.gstatic.com";
  if (url.origin !== self.location.origin && !fonts) return;
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
      const fresh = fetch(request).then((response) => keep(request, response)).catch(() => cached);
      return cached || fresh;
    }),
  );
});
