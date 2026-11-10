# Site do cppm

Landing page em Next.js, com exportação estática, documentação e download verificado.

```sh
cd website
npm ci
npm run dev
```

O servidor de desenvolvimento abre em `http://127.0.0.1:3000`.

A documentação tem 18 tópicos em quatro grupos, incluindo um guia Windows.
O catálogo e as versões são
copiados de `src/packages.rs` do cppm pelo script de sincronização, evitando
manter uma lista de versões independente no site. Os guias Markdown também
acompanham o download nativo.

```sh
npm run check
npm run build
npm start -- --port 3045
```

Com a prévia em execução, `CPPM_PREVIEW_URL=http://127.0.0.1:3045 npm test` verifica
navegação, animações, tamanhos de tela, busca e os checksums dos downloads Linux/Windows usando
Chromium do Playwright. As capturas ficam em `/tmp/cppm-website-qa`.

Antes de iniciar ou compilar, `scripts/sync-project.mjs` copia os documentos e
pacotes disponíveis no projeto Rust. A cópia incluída no site permite compilar
o site de forma independente.
