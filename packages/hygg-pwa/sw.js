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
// the Pages site serves many deploys from one origin: the latest build at
// /hygg/, every tagged build at /hygg/<tag>/, and the main branch at
// /hygg/main/. CacheStorage is per-origin, not per-scope, so an unqualified
// cache name would leave those deploys sharing — and evicting — one another's
// entries.

// This deploy's root: an absolute URL ending in "/", e.g.
// "https://kruseio.github.io/hygg/v0.1.21/". Doubles as the app-shell cache key,
// since the shell is what a navigation falls back to when offline.
const SCOPE = self.registration.scope;
const CACHE_PREFIX = "hygg-cache-";
const CACHE = `${CACHE_PREFIX}v3:${SCOPE}`;

// A sibling deploy — a pinned release (/hygg/0.1.25/) or the main channel
// (/hygg/main/) — sits *inside* the latest deploy's scope (/hygg/), so the
// latest worker sees its requests until that deploy's own worker takes over.
// They are not ours: caching a sibling's HTML under our shell key would serve
// it at the root URL when offline. The more specific registration wins once it
// exists, so this only covers the first load — which is exactly when the damage
// would be done. The first path segment past the scope names the sibling:
// `main`, or a bare version like `0.1.25` (this repo tags 0.1.25, not v0.1.25,
// so the old `v\d` test matched neither `main` nor any tag and let both leak
// into this cache).
function belongsToAnotherDeploy(url) {
  if (!url.href.startsWith(SCOPE)) return false;
  const seg = url.href.slice(SCOPE.length).split("/")[0];
  return seg === "main" || /^v?\d+\.\d+\.\d+/.test(seg);
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

  // The version manifest is a mutable pointer, like the HTML shell: the in-app
  // update checker polls it to learn when a newer build has shipped, so a cached
  // copy would freeze it at this build's view of "latest" and no update would
  // ever surface. Network-first, keeping the last good copy under a query-
  // stripped key as an offline fallback — the checker cache-busts with a query
  // string, which would otherwise leave a dead cache entry per poll.
  if (url.pathname.endsWith("/versions.json")) {
    event.respondWith(
      (async () => {
        const cache = await caches.open(CACHE);
        const key = url.origin + url.pathname;
        try {
          const res = await fetch(req);
          if (res && res.ok) cache.put(key, res.clone());
          return res;
        } catch (err) {
          return (await cache.match(key)) || Response.error();
        }
      })(),
    );
    return;
  }

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
