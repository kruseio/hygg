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

const CACHE = "hygg-cache-v2";

self.addEventListener("install", (event) => {
  event.waitUntil(
    caches.open(CACHE).then((cache) => cache.addAll(["/"])),
  );
  self.skipWaiting();
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    (async () => {
      const keys = await caches.keys();
      await Promise.all(
        keys.filter((k) => k !== CACHE).map((k) => caches.delete(k)),
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

  if (req.mode === "navigate") {
    event.respondWith(
      (async () => {
        const cache = await caches.open(CACHE);
        try {
          const res = await fetch(req);
          if (res && res.ok) cache.put("/", res.clone());
          return res;
        } catch (err) {
          return (
            (await cache.match("/")) ||
            (await cache.match("/index.html")) ||
            Response.error()
          );
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
