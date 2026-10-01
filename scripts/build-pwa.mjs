import { readdir, readFile, writeFile } from "node:fs/promises";
import { createHash } from "node:crypto";
const files = [];
async function walk(dir, prefix = "") {
  for (const f of await readdir(dir, { withFileTypes: true })) {
    if (f.isDirectory()) await walk(`${dir}/${f.name}`, `${prefix}/${f.name}`);
    else if (f.name !== "sw.js") files.push(`${prefix}/${f.name}`);
  }
}
await walk("dist");
const version = createHash("sha256");
for (const file of files) version.update(await readFile(`dist${file}`));
await writeFile(
  "dist/sw.js",
  `const CACHE='chores-${version.digest("hex").slice(0, 16)}';const FILES=${JSON.stringify(files)};
self.addEventListener('install',e=>e.waitUntil(caches.open(CACHE).then(c=>c.addAll(FILES))));
self.addEventListener('activate',e=>e.waitUntil(caches.keys().then(keys=>Promise.all(keys.filter(k=>k.startsWith('chores-')&&k!==CACHE).map(k=>caches.delete(k))))));
self.addEventListener('fetch',e=>{const u=new URL(e.request.url);if(u.origin!==self.location.origin||u.pathname.startsWith('/api')||e.request.method!=='GET')return;if(e.request.mode==='navigate'){e.respondWith(fetch(e.request).catch(()=>caches.match('/index.html')));return;}e.respondWith(caches.match(e.request).then(c=>c||fetch(e.request)));});`,
);
