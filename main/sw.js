// hygg-pwa service worker — offline app-shell caching.
//
// Strategy split by resource kind, which matters because Trunk fingerprints the
// wasm/js/css (immutable) but the HTML shell is not fingerprinted and carries
// subresource-integrity hashes pointing at the current build:
//   - HTML navigations → network-first (always pick up a fresh deploy; the SRI
//     hashes then match the assets it references), falling back to the cached
//     shell only when offline;
//   - everything else (hashed assets, icons, manifest) → cache-first, since a
//     content-hashed URL never changes meaning.
//
// Everything below keys off `self.registration.scope` rather than "/", because
// the Pages site serves many deploys from one origin: the latest build at /hygg/
// and every tagged build at /hygg/v<tag>/. CacheStorage is per-origin, not
// per-scope, so an unqualified cache name would leave those deploys sharing —
// and evicting — one another's entries.

// This deploy's root: an absolute URL ending in "/", e.g.
// "https://kruseio.github.io/hygg/v0.1.21/". Doubles as the app-shell cache key,
// since the shell is what a navigation falls back to when offline.
const SCOPE = self.registration.scope;
const CACHE_PREFIX = "hygg-cache-";
const CACHE = `${CACHE_PREFIX}v3:${SCOPE}`;

// A pinned deploy (/hygg/v0.1.21/) sits *inside* the latest deploy's scope
// (/hygg/), so the latest worker sees the pinned deploy's requests until that
// deploy's own worker takes over. They are not ours: caching a pinned build's
// HTML under our shell key would serve v0.1.21 at the root URL when offline. The
// more specific registration wins once it exists, so this only covers the first
// load — which is exactly when the damage would be done.
function belongsToAnotherDeploy(url) {
  if (!url.href.startsWith(SCOPE)) return false;
  return /^v\d[^/]*\//.test(url.href.slice(SCOPE.length));
}

self.addEventListener("install", (event) => {
  event.waitUntil(caches.open(CACHE).then((cache) => cache.addAll([SCOPE])));
  self.skipWaiting();
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    (async () => {
      const keys = await caches.keys();
      await Promise.all(
        keys
          // Only ever drop our *own* superseded caches. Another deploy's cache
          // is another deploy's business — deleting it would silently break a
          // pinned version's offline mode. Legacy unscoped names (pre-v3, no
          // ":scope" suffix) predate versioned deploys, so they are ours.
          .filter((k) => {
            if (!k.startsWith(CACHE_PREFIX)) return false;
            return k.includes(":") ? k.endsWith(`:${SCOPE}`) && k !== CACHE : true;
          })
          .map((k) => caches.delete(k)),
      );
      await self.clients.claim();
    })(),
  );
});

self.addEventListener("fetch", (event) => {
  const req = event.request;
  if (req.method !== "GET") return;
  const url = new URL(req.url);
  if (url.origin !== self.location.origin) return;
  if (belongsToAnotherDeploy(url)) return;

  if (req.mode === "navigate") {
    event.respondWith(
      (async () => {
        const cache = await caches.open(CACHE);
        try {
          const res = await fetch(req);
          if (res && res.ok) cache.put(SCOPE, res.clone());
          return res;
        } catch (err) {
          return (await cache.match(SCOPE)) || Response.error();
        }
      })(),
    );
    return;
  }

  event.respondWith(
    (async () => {
      const cache = await caches.open(CACHE);
      const cached = await cache.match(req);
      if (cached) return cached;
      const res = await fetch(req);
      if (res && res.ok) cache.put(req, res.clone());
      return res;
    })(),
  );
});
