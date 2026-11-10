import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { chromium, expect } from "@playwright/test";

const baseURL = process.env.CPPM_PREVIEW_URL ?? "http://127.0.0.1:3000";
const output = path.resolve(process.env.CPPM_QA_DIR ?? "/tmp/cppm-website-qa");
await mkdir(output, { recursive: true });
const browser = await chromium.launch();

try {
  const context = await browser.newContext({ viewport: { width: process.argv.includes("--narrow") ? 320 : process.argv.includes("--mobile") ? 390 : 1440, height: 1000 }, permissions: ["clipboard-read", "clipboard-write"] });
  const page = await context.newPage();
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  page.on("console", (message) => { if (message.type() === "error") errors.push(message.text()); });
  await page.goto(baseURL, { waitUntil: "networkidle" });
  if (process.argv.includes("--catalog")) {
    await page.goto(`${baseURL}/#docs-catalogo`, { waitUntil: "networkidle" });
    await expect(page.locator(".doc-article h3")).toHaveText("Catálogo");
  }
  await page.evaluate(() => document.fonts.ready);
  await expect(page.locator(".terminal-status")).toHaveClass(/is-complete/);
  if (process.argv.includes("--mobile")) {
    await page.getByRole("button", { name: "Abrir menu", exact: true }).click();
    await page.waitForTimeout(300);
    await page.screenshot({ path: path.join(output, "mobile-inspect.png") });
  }
  await page.screenshot({ path: path.join(output, "desktop.png") });
  if (process.argv.includes("--inspect")) {
    console.log("overflowing roots", await page.evaluate(() => [...document.querySelectorAll(".docs-workspace *")].filter((element) => { const rect = element.getBoundingClientRect(); return rect.right > innerWidth + 1 && !element.closest(".doc-table-scroll"); }).map((element) => ({ tag: element.tagName, class: element.className, right: element.getBoundingClientRect().right, position: getComputedStyle(element).position, after: getComputedStyle(element, "::after").width, before: getComputedStyle(element, "::before").width })).slice(0, 10)));
    console.log(JSON.stringify(await page.locator(".docs-sidebar, .docs-sidebar-inner, .docs-sidebar-title, .docs-mobile-select, .docs-mobile-select select, .docs-sidebar-bottom, .docs-filter, .docs-tabs").evaluateAll((elements) => elements.map((element) => ({ class: element.className, rect: element.getBoundingClientRect().toJSON(), width: element.clientWidth, scrollWidth: element.scrollWidth, overflow: getComputedStyle(element).overflow, display: getComputedStyle(element).display, minWidth: getComputedStyle(element).minWidth }))), null, 2));
    console.log(JSON.stringify(await page.locator("html, body, .docs-workspace, .doc-content, .doc-article, .library-catalog, .doc-table-scroll, .catalog-group, .doc-table").evaluateAll((elements) => elements.map((element) => ({ class: element.className, width: element.clientWidth, scrollWidth: element.scrollWidth, rect: element.getBoundingClientRect().toJSON(), minWidth: getComputedStyle(element).minWidth, display: getComputedStyle(element).display }))), null, 2));
    console.log(JSON.stringify(await page.locator(".site-header, .header-inner, .header-download, .desktop-nav, .mobile-nav, .terminal-tabs, .hero-line").evaluateAll((elements) => elements.map((element) => {
      const style = getComputedStyle(element);
      return { class: element.className, rect: element.getBoundingClientRect().toJSON(), display: style.display, position: style.position, color: style.color, font: style.font, background: style.backgroundColor, opacity: style.opacity };
    })), null, 2));
    console.log(await page.evaluate(() => [...document.styleSheets].map((sheet) => ({ href: sheet.href, rules: [...sheet.cssRules].slice(0, 4).map((rule) => rule.cssText.slice(0, 500)) }))));
    process.exitCode = 0;
  } else {
    await expect(page).toHaveTitle("cppm — Menos setup. Mais C++.");
    assert.equal(await page.locator(".site-header").evaluate((element) => getComputedStyle(element).position), "fixed");
    const headingColors = await page.locator(".hero-line").evaluateAll((elements) => elements.map((element) => getComputedStyle(element).color));
    assert.notEqual(headingColors[0], headingColors[1], "The two heading lines must keep their individual colors.");

    // Tab selection, rapid changes and replay must resolve to the selected example.
    const examples = [
      { name: "API HTTP", folder: "minha-api", command: "--template drogon", lines: 3 },
      { name: "WebAssembly", folder: "meu-wasm", command: "--template wasm", lines: 3 },
      { name: "App C++", folder: "meu-app", command: "cppm init meu-app", lines: 4 },
    ];
    const terminalHeight = await page.locator(".terminal-window").evaluate((element) => element.offsetHeight);
    for (const example of examples) {
      await page.getByRole("tab", { name: example.name, exact: true }).click();
      await expect(page.locator(".terminal-step.is-shown")).toHaveCount(1);
      await expect(page.locator(".terminal-titlebar")).toContainText(example.folder);
      await expect(page.locator("#terminal-panel")).toContainText(example.command);
      await expect(page.locator(".terminal-status")).toHaveClass(/is-complete/);
      await expect(page.locator(".terminal-step.is-shown")).toHaveCount(example.lines);
      assert.equal(await page.locator(".terminal-window").evaluate((element) => element.offsetHeight), terminalHeight, "Switching templates must keep the terminal height stable.");
    }
    for (const name of ["API HTTP", "WebAssembly", "App C++", "WebAssembly", "API HTTP", "WebAssembly"]) {
      await page.getByRole("tab", { name, exact: true }).click();
    }
    await expect(page.locator(".terminal-status")).toHaveClass(/is-complete/);
    await expect(page.locator(".terminal-titlebar")).toContainText("meu-wasm");
    await expect(page.locator(".terminal-step.is-shown")).toHaveCount(3);
    await expect(page.locator(".terminal-result")).toContainText("C++ também vive no navegador.");
    await page.getByRole("button", { name: "Repetir animação do terminal" }).click();
    await expect(page.locator(".terminal-step.is-shown")).toHaveCount(1);
    await expect(page.locator(".terminal-status")).toHaveClass(/is-complete/);
    await page.getByRole("tab", { name: "WebAssembly", exact: true }).focus();
    await page.keyboard.press("ArrowLeft");
    await expect(page.getByRole("tab", { name: "API HTTP", exact: true })).toBeFocused();
    await expect(page.getByRole("tab", { name: "API HTTP", exact: true })).toHaveAttribute("aria-selected", "true");

    // Reduced motion must also take effect during a running animation.
    await page.emulateMedia({ reducedMotion: "reduce" });
    await expect(page.locator(".terminal-status")).toHaveClass(/is-complete/);
    await page.getByRole("tab", { name: "App C++", exact: true }).click();
    await expect(page.locator(".terminal-step.is-shown")).toHaveCount(4);
    await page.getByRole("button", { name: "Copiar comandos do exemplo" }).click();
    assert.equal(await page.evaluate(() => navigator.clipboard.readText()), "cppm init meu-app\ncd meu-app\ncppm add fmt\ncppm run dev");

    // Search must select the correct topic, including keyboard and no-result paths.
    await page.keyboard.press("Control+k");
    const search = page.getByRole("dialog", { name: "Buscar na documentação" });
    await expect(search).toBeVisible();
    await page.getByRole("textbox", { name: "Buscar um tópico ou comando" }).fill("sqlite");
    await search.getByRole("button", { name: /Dependências/ }).click();
    await expect(search).not.toBeVisible();
    await expect(page.locator(".doc-article h3")).toHaveText("Dependências");
    await expect(page).toHaveURL(/#docs-dependencias$/);
    await page.getByRole("button", { name: "Buscar na documentação" }).click();
    await page.getByRole("textbox", { name: "Buscar um tópico ou comando" }).fill("zzzzxxxyyy");
    await expect(search).toContainText("Nenhum tópico encontrado.");
    await page.keyboard.press("Escape");
    await expect(search).not.toBeVisible();

    const nativeProject = JSON.parse(await readFile(new URL("../src/generated/project.json", import.meta.url), "utf8"));
    await expect(page.locator(".docs-group-label")).toHaveText(["Começar", "No projeto", "Bibliotecas", "Avançado"]);
    const topicTabs = page.locator(".docs-tabs [role=tab]");
    const topicCount = await topicTabs.count();
    assert.equal(topicCount, 18);
    for (let index = 0; index < topicCount; index++) {
      const tab = topicTabs.nth(index);
      const title = await tab.innerText();
      await tab.click();
      await expect(page.locator(".doc-article h3")).toHaveText(title.trim());
    }
    await page.getByRole("tab", { name: "Catálogo", exact: true }).click();
    await expect(page.locator(".catalog-table tbody tr")).toHaveCount(nativeProject.catalog.length);
    await expect(page.locator(".catalog-group")).toHaveCount(7);
    await page.getByRole("button", { name: "Copiar: cppm install opencv", exact: true }).click();
    assert.equal(await page.evaluate(() => navigator.clipboard.readText()), "cppm install opencv");
    const topicFilter = page.getByRole("textbox", { name: "Filtrar tópicos da documentação" });
    await topicFilter.fill("Eigen");
    await expect(page.getByRole("tab", { name: "Imagens e matemática", exact: true })).toBeVisible();
    await expect(page.getByRole("tab", { name: "Instalação", exact: true })).toHaveCount(0);
    await page.getByRole("button", { name: "Limpar filtro de tópicos" }).click();
    await expect(topicTabs).toHaveCount(topicCount);
    // The new product links must open searchable, deep-linkable documentation.
    await page.locator('.update-doc-link[href="#docs-interativo"]').click();
    await expect(page.locator(".doc-article h3")).toHaveText("Interface interativa");
    await expect(page.locator(".doc-article")).toContainText("cppm --offline ui");
    await page.locator('.update-doc-link[href="#docs-protecao"]').click();
    await expect(page.locator(".doc-article h3")).toHaveText("Build ofuscado");
    await expect(page.locator(".doc-article")).toContainText("--obf-basic");
    await expect(page.locator(".doc-article")).toContainText("LLVM 18–23");
    await page.getByRole("button", { name: "Buscar na documentação", exact: true }).click();
    await page.getByRole("textbox", { name: "Buscar um tópico ou comando" }).fill("ofuscação");
    await search.getByRole("button", { name: /Build ofuscado/ }).click();
    await expect(page).toHaveURL(/#docs-protecao$/);
    await page.getByRole("button", { name: "Copiar: cppm ui", exact: true }).click();
    assert.equal(await page.evaluate(() => navigator.clipboard.readText()), "cppm ui");
    await page.getByRole("button", { name: "Copiar: cppm build obf", exact: true }).click();
    assert.equal(await page.evaluate(() => navigator.clipboard.readText()), "cppm build obf");
    await page.locator(".updates-section").scrollIntoViewIfNeeded();
    await page.screenshot({ path: path.join(output, "updates-desktop.png"), animations: "disabled" });
    await page.setViewportSize({ width: 390, height: 1000 });
    await page.locator(".updates-section").scrollIntoViewIfNeeded();
    await page.screenshot({ path: path.join(output, "updates-mobile.png"), animations: "disabled" });
    await page.setViewportSize({ width: 1440, height: 1000 });
    await page.goto(`${baseURL}/#docs-protecao/basico`, { waitUntil: "networkidle" });
    await expect(page.locator("#doc-protecao-basico")).toBeInViewport();
    await page.reload({ waitUntil: "networkidle" });
    await expect(page.locator(".doc-article h3")).toHaveText("Build ofuscado");
    await expect(page.locator("#doc-protecao-basico")).toBeInViewport();
    await page.locator(".docs-workspace").scrollIntoViewIfNeeded();
    await page.screenshot({ path: path.join(output, "documentation-desktop.png"), animations: "disabled" });
    for (const width of [320, 390, 760, 820, 1440]) {
      await page.setViewportSize({ width, height: 1000 });
      assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1), `Catalog overflow at ${width}px`);
      if (width === 390) {
        await expect(page.locator(".docs-mobile-select select optgroup")).toHaveCount(4);
        await page.locator(".docs-workspace").scrollIntoViewIfNeeded();
        await page.screenshot({ path: path.join(output, "documentation-mobile.png"), animations: "disabled" });
      }
    }
    await page.goto(`${baseURL}/#docs-bancos/postgres`, { waitUntil: "networkidle" });
    await expect(page.locator(".doc-article h3")).toHaveText("Bancos de dados");
    await expect(page.locator("#doc-bancos-postgres")).toBeInViewport();
    await page.reload({ waitUntil: "networkidle" });
    await expect(page.locator(".doc-article h3")).toHaveText("Bancos de dados");
    await expect(page.locator("#doc-bancos-postgres")).toBeInViewport();

    // Verify the actual downloaded binary against the bundled SHA-256.
    await page.locator(".platform-tabs").getByRole("button", { name: "Linux", exact: true }).click();
    const downloadLink = page.getByRole("link", { name: "Baixar para Linux", exact: true });
    const [download] = await Promise.all([page.waitForEvent("download"), downloadLink.click()]);
    const manifest = nativeProject;
    const artifact = manifest.downloads.find((item) => item.platform === "linux");
    assert.equal(download.suggestedFilename(), artifact.filename);
    assert.equal(createHash("sha256").update(await readFile(await download.path())).digest("hex"), artifact.sha256);
    await page.getByRole("button", { name: "Já baixou? Veja como instalar" }).click();
    await expect(page.locator("#download-install-instructions")).toContainText(`tar -xzf ${artifact.filename}`);
    await page.locator(".platform-tabs").getByRole("button", { name: "Windows", exact: true }).click();
    const windowsArtifact = manifest.downloads.find((item) => item.platform === "windows");
    let windowsDownloadSha256 = null;
    if (windowsArtifact) {
      const [windowsDownload] = await Promise.all([page.waitForEvent("download"), page.getByRole("link", { name: "Baixar para Windows", exact: true }).click()]);
      assert.equal(windowsDownload.suggestedFilename(), windowsArtifact.filename);
      windowsDownloadSha256 = createHash("sha256").update(await readFile(await windowsDownload.path())).digest("hex");
      assert.equal(windowsDownloadSha256, windowsArtifact.sha256);
      await page.getByRole("button", { name: "Já baixou? Veja como instalar", exact: true }).click();
      await expect(page.locator("#download-install-instructions")).toContainText(".\\install.ps1");
      await page.getByRole("button", { name: "Guia Windows", exact: true }).click();
    } else {
      await expect(page.locator(".download-description")).toContainText("O pacote Windows ainda não está disponível");
      await page.getByRole("button", { name: "Ver instruções de instalação", exact: true }).click();
    }
    await expect(page.locator(".doc-article h3")).toHaveText(windowsArtifact ? "Windows" : "Instalação");
    await page.locator(".platform-tabs").getByRole("button", { name: "Linux", exact: true }).click();

    // Catch the original cascade issue and header collisions across breakpoints.
    const layouts = [];
    for (const width of [320, 360, 390, 760, 768, 820, 960, 1024, 1280, 1440, 1920]) {
      await page.setViewportSize({ width, height: 1000 });
      await page.evaluate(() => window.scrollTo({ top: 0, behavior: "instant" }));
      await page.mouse.move(0, 0);
      await page.waitForTimeout(100);
      const layout = await page.evaluate(() => {
        const header = document.querySelector(".header-inner");
        const rect = header.getBoundingClientRect();
        const controls = [...header.querySelectorAll("a, button")].filter((element) => element.getClientRects().length).map((element) => ({ name: element.getAttribute("aria-label") || element.textContent.trim(), ...element.getBoundingClientRect().toJSON() }));
        const tabs = [...document.querySelectorAll(".terminal-tabs button")].map((element) => ({ ...element.getBoundingClientRect().toJSON(), layoutWidth: element.offsetWidth }));
        return { width: innerWidth, scrollWidth: document.documentElement.scrollWidth, header: rect.toJSON(), controls, tabs, stackFont: getComputedStyle(document.querySelector(".stack-caption")).fontSize };
      });
      assert.ok(layout.scrollWidth <= width + 1, `Horizontal overflow at ${width}px: ${layout.scrollWidth}`);
      for (let index = 0; index < layout.controls.length; index++) {
        const control = layout.controls[index];
        assert.ok(control.left >= layout.header.left - 1 && control.right <= layout.header.right + 1, `${control.name} escapes the header at ${width}px`);
        for (const next of layout.controls.slice(index + 1)) {
          assert.ok(control.right <= next.left + 1 || next.right <= control.left + 1, `${control.name} overlaps ${next.name} at ${width}px`);
        }
      }
      assert.ok(Math.max(...layout.tabs.map((tab) => tab.layoutWidth)) - Math.min(...layout.tabs.map((tab) => tab.layoutWidth)) <= 1, `Uneven terminal tabs at ${width}px: ${JSON.stringify(layout.tabs)}`);
      assert.ok(parseFloat(layout.stackFont) >= 12, `The tool descriptions are too small at ${width}px`);
      layouts.push(layout);
      if (width === 390 || width === 820) await page.screenshot({ path: path.join(output, `${width === 390 ? "mobile" : "tablet"}.png`) });
    }

    await page.setViewportSize({ width: 390, height: 844 });
    await page.getByRole("button", { name: "Abrir menu", exact: true }).click();
    await expect(page.getByRole("navigation", { name: "Navegação mobile" })).toBeVisible();
    await expect(page.getByRole("navigation", { name: "Navegação mobile" })).toHaveCSS("opacity", "1");
    await expect(page.getByRole("navigation", { name: "Navegação mobile" })).toBeInViewport();
    await page.screenshot({ path: path.join(output, "mobile-menu.png"), animations: "disabled" });
    await page.getByRole("navigation", { name: "Navegação mobile" }).getByRole("link", { name: "Documentação", exact: true }).click();
    await expect(page.getByRole("navigation", { name: "Navegação mobile" })).toHaveCount(0);
    await page.locator(".docs-mobile-select select").selectOption("manifesto");
    await expect(page.locator(".doc-article h3")).toHaveText("cppm.toml");
    await expect(page).toHaveURL(/#docs-manifesto$/);
    await page.reload({ waitUntil: "networkidle" });
    await expect(page.locator(".doc-article h3")).toHaveText("cppm.toml");

    await page.setViewportSize({ width: 1440, height: 1000 });
    await page.goto(baseURL, { waitUntil: "networkidle" });
    await expect(page.locator(".terminal-status")).toHaveClass(/is-complete/);
    await page.screenshot({ path: path.join(output, "desktop.png"), fullPage: true });
    const linkStatuses = [];
    for (const url of ["/docs/README.md", "/docs/installation.md", "/docs/architecture.md", "/docs/packages.md", "/docs/guide.md", "/docs/windows.md", "/LICENSE.txt", "/THIRD_PARTY_NOTICES.txt", artifact.checksumUrl, ...(windowsArtifact ? [windowsArtifact.checksumUrl] : [])]) {
      const response = await context.request.get(new URL(url, baseURL).href);
      assert.equal(response.status(), 200, `Missing resource: ${url}`);
      linkStatuses.push({ url, status: response.status() });
    }
    assert.deepEqual(errors, [], "The browser reported errors.");
    await writeFile(path.join(output, "report.json"), `${JSON.stringify({ baseURL, layouts, linkStatuses, errors, downloadSha256: artifact.sha256, windowsDownloadSha256 }, null, 2)}\n`);
    console.log(`Passed: terminal animation, ${topicCount} documentation topics, ${nativeProject.catalog.length} catalog entries, section links, filtering, search, clipboard, download checksum and ${layouts.length} viewport sizes. Screenshots: ${output}`);
  }
} finally {
  await browser.close();
}
