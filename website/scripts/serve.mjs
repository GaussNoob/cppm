import { createServer } from "node:http";
import { readFile, stat } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(fileURLToPath(new URL("../out/", import.meta.url)));
const portArgument = process.argv.indexOf("--port");
const port = Number(portArgument >= 0 ? process.argv[portArgument + 1] : process.env.PORT ?? 3000);
if (!Number.isInteger(port) || port < 1 || port > 65535) throw new Error("Use a port between 1 and 65535.");
await stat(path.join(root, "index.html")).catch(() => { throw new Error("Run npm run build before npm start."); });

const types = {
  ".html": "text/html; charset=utf-8", ".js": "text/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8", ".json": "application/json; charset=utf-8",
  ".svg": "image/svg+xml", ".png": "image/png", ".ico": "image/x-icon",
  ".woff2": "font/woff2", ".md": "text/markdown; charset=utf-8",
  ".txt": "text/plain; charset=utf-8", ".sha256": "text/plain; charset=utf-8",
  ".gz": "application/gzip", ".zip": "application/zip",
};

const server = createServer(async (request, response) => {
  if (request.method !== "GET" && request.method !== "HEAD") {
    response.writeHead(405, { Allow: "GET, HEAD" }).end();
    return;
  }
  try {
    const pathname = decodeURIComponent(new URL(request.url, "http://localhost").pathname);
    let filename = path.resolve(root, `.${pathname}`);
    if (filename !== root && !filename.startsWith(`${root}${path.sep}`)) { response.writeHead(403).end(); return; }
    const info = await stat(filename);
    if (info.isDirectory()) filename = path.join(filename, "index.html");
    const content = await readFile(filename);
    response.writeHead(200, { "Content-Type": types[path.extname(filename)] ?? "application/octet-stream", "Content-Length": content.length, "Cache-Control": "no-store" });
    response.end(request.method === "HEAD" ? undefined : content);
  } catch (error) {
    if (error instanceof URIError) { response.writeHead(400).end(); return; }
    const content = await readFile(path.join(root, "404.html")).catch(() => Buffer.from("Not found"));
    response.writeHead(404, { "Content-Type": "text/html; charset=utf-8" });
    response.end(request.method === "HEAD" ? undefined : content);
  }
});

server.listen(port, "127.0.0.1", () => console.log(`Local: http://127.0.0.1:${port}`));
process.once("SIGTERM", () => server.close());
process.once("SIGINT", () => server.close());
