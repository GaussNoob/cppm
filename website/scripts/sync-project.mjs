import { createHash } from "node:crypto";
import { copyFile, mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const siteRoot = fileURLToPath(new URL("../", import.meta.url));
const projectRoot = path.resolve(siteRoot, "..");
const generated = path.join(siteRoot, "src/generated/project.json");
let cargo;

try {
  cargo = await readFile(path.join(projectRoot, "Cargo.toml"), "utf8");
} catch (error) {
  if (error.code !== "ENOENT") throw error;
  // The website is also a standalone, deployable project.
  await readFile(generated);
  console.log("Using the bundled cppm documentation and downloads.");
  process.exit(0);
}

const version = cargo.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
if (!version) throw new Error("Could not read the cppm version from Cargo.toml.");

const downloadDir = path.join(siteRoot, "public/downloads");
const docsDir = path.join(siteRoot, "public/docs");
await Promise.all([
  mkdir(downloadDir, { recursive: true }),
  mkdir(docsDir, { recursive: true }),
  mkdir(path.dirname(generated), { recursive: true }),
]);

await Promise.all([
  copyFile(path.join(projectRoot, "README.md"), path.join(docsDir, "README.md")),
  copyFile(path.join(projectRoot, "docs/installation.md"), path.join(docsDir, "installation.md")),
  copyFile(path.join(projectRoot, "docs/architecture.md"), path.join(docsDir, "architecture.md")),
  copyFile(path.join(projectRoot, "docs/performance.md"), path.join(docsDir, "performance.md")),
  copyFile(path.join(projectRoot, "docs/packages.md"), path.join(docsDir, "packages.md")),
  copyFile(path.join(projectRoot, "docs/guide.md"), path.join(docsDir, "guide.md")),
  copyFile(path.join(projectRoot, "docs/windows.md"), path.join(docsDir, "windows.md")),
  copyFile(path.join(projectRoot, "docs/roadmap.md"), path.join(docsDir, "roadmap.md")),
  copyFile(path.join(projectRoot, "LICENSE"), path.join(siteRoot, "public/LICENSE.txt")),
  copyFile(path.join(projectRoot, "THIRD_PARTY_NOTICES.txt"), path.join(siteRoot, "public/THIRD_PARTY_NOTICES.txt")),
]);

const dist = path.join(projectRoot, "dist");
const filenames = await readdir(dist).catch((error) => {
  if (error.code === "ENOENT") return [];
  throw error;
});
const bundled = JSON.parse(await readFile(generated, "utf8"));
const downloads = [];

for (const filename of filenames.sort()) {
  if (!filename.startsWith(`cppm-${version}-`)) continue;
  const match = filename.match(/-(linux|windows)-(x86_64|arm64)\.(tar\.gz|zip)$/);
  if (!match) continue;
  const bytes = await readFile(path.join(dist, filename));
  const sha256 = createHash("sha256").update(bytes).digest("hex");
  const checksumName = `${filename}.sha256`;
  const expected = (await readFile(path.join(dist, checksumName), "utf8")).split(/\s+/)[0];
  if (sha256 !== expected) throw new Error(`Invalid checksum for ${filename}`);
  await Promise.all([
    copyFile(path.join(dist, filename), path.join(downloadDir, filename)),
    copyFile(path.join(dist, checksumName), path.join(downloadDir, checksumName)),
  ]);
  downloads.push({
    platform: match[1],
    architecture: match[2],
    filename,
    url: `/downloads/${filename}`,
    checksumUrl: `/downloads/${checksumName}`,
    sha256,
    size: bytes.length,
  });
}

const packageSource = await readFile(path.join(projectRoot, "src/packages.rs"), "utf8");
const catalogSource = packageSource.match(/pub const CATALOG:[\s\S]*?= &\[([\s\S]*?)\n\];/)?.[1];
const aliasSource = packageSource.match(/pub const ALIASES:[\s\S]*?= &\[([\s\S]*?)\n\];/)?.[1];
if (!catalogSource || !aliasSource) throw new Error("Could not read the native cppm catalog.");
const catalog = [...catalogSource.matchAll(/PackageInfo\s*\{\s*name:\s*"([^"]+)",\s*version:\s*"([^"]+)",\s*cmake_package:\s*"([^"]+)",\s*target:\s*"([^"]+)",?\s*\}/g)]
  .map(([, name, version, cmakePackage, target]) => ({ name, version, cmakePackage, target }));
if (!catalog.length || catalog.length !== (catalogSource.match(/PackageInfo\s*\{/g) ?? []).length) throw new Error("Incomplete native cppm catalog.");
const aliases = [...aliasSource.matchAll(/\("([^"]+)",\s*"([^"]+)"\)/g)].map(([, name, packageName]) => ({ name, package: packageName }));
if (!downloads.length) {
  for (const artifact of bundled.downloads) {
    const bytes = await readFile(path.join(downloadDir, artifact.filename));
    if (createHash("sha256").update(bytes).digest("hex") !== artifact.sha256) {
      throw new Error(`Invalid bundled checksum for ${artifact.filename}`);
    }
    downloads.push(artifact);
  }
}
await writeFile(generated, `${JSON.stringify({ version, downloads, catalog, aliases }, null, 2)}\n`);
console.log(`Synced cppm ${version}: ${catalog.length} packages, documentation and ${downloads.length} verified download(s).`);
