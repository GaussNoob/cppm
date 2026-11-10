"use client";

import { ArrowDown, ArrowDownToLine, ArrowRight, ArrowUpRight, Check, ChevronDown, Code2, Monitor, Package, ShieldCheck, Terminal } from "lucide-react";
import { useEffect, useState } from "react";
import { openDoc } from "@/lib/docs";
import { CodeBlock } from "./ui";

type Artifact = { platform: string; architecture: string; filename: string; url: string; checksumUrl: string; sha256: string; size: number };

export function Download({ version: projectVersion, downloads }: { version: string; downloads: Artifact[] }) {
  const [platform, setPlatform] = useState<"linux" | "windows">("linux");
  const [architecture, setArchitecture] = useState("x86_64");
  const [showInstall, setShowInstall] = useState(false);
  const platformArtifacts = downloads.filter((download) => download.platform === platform);
  const version = platformArtifacts[0]?.filename.match(/^cppm-(.+?)-(?:linux|windows)-/)?.[1] ?? projectVersion;
  const artifact = platformArtifacts.find((download) => download.architecture === architecture);

  useEffect(() => {
    if (/windows/i.test(navigator.userAgent)) setPlatform("windows");
  }, []);

  function changePlatform(value: "linux" | "windows") {
    setPlatform(value);
    setShowInstall(false);
    setArchitecture(downloads.find((download) => download.platform === value)?.architecture ?? "x86_64");
  }

  const installCode = artifact ? platform === "linux"
    ? `tar -xzf ${artifact.filename}\ncd ${artifact.filename.replace(/\.tar\.gz$/, "")}\nsh install.sh\ncppm doctor`
    : `# Extraia o arquivo ${artifact.filename}\n# Na pasta extraída, execute no PowerShell:\npowershell -NoProfile -ExecutionPolicy Bypass -File .\\install.ps1\ncppm doctor`
    : "cargo build --release --locked\npowershell -NoProfile -ExecutionPolicy Bypass -File .\\scripts\\install.ps1";

  return (
    <section className="download-section section-space" id="download" aria-labelledby="download-heading">
      <div className="container">
        <div className="download-layout reveal">
          <div className="download-intro"><span className="eyebrow"><span className="section-index">04 /</span> DOWNLOAD</span><h2 id="download-heading">Seu próximo<br />projeto começa<br /><span>aqui<span className="accent-period">.</span></span></h2><p>Um executável. O seu terminal.<br />E tudo o que você quer construir.</p><div className="download-license"><span className="status-dot done" />Open source<span className="dot-separator">·</span>Licença MIT<span className="dot-separator">·</span>v{version}</div></div>
          <div className="download-panel">
            <div className="download-panel-top"><span><Package size={16} />cppm / v{version}</span><span className="release-tag">PRÉ-1.0</span></div>
            <div className="platform-tabs" role="group" aria-label="Sistema operacional"><button type="button" aria-pressed={platform === "linux"} className={platform === "linux" ? "is-active" : ""} onClick={() => changePlatform("linux")}><Terminal size={16} />Linux</button><button type="button" aria-pressed={platform === "windows"} className={platform === "windows" ? "is-active" : ""} onClick={() => changePlatform("windows")}><Monitor size={16} />Windows</button></div>
            <div className="download-panel-body">
              <div className="download-package"><div className="download-package-icon">{platform === "linux" ? <Terminal size={28} strokeWidth={1.4} /> : <Monitor size={28} strokeWidth={1.4} />}</div><div><h3>cppm para {platform === "linux" ? "Linux" : "Windows"}</h3><span>{artifact ? `v${version} · ${artifact.architecture === "arm64" ? "ARM64" : "x86_64"} · ${Math.ceil(artifact.size / 1024)} KB` : "Instalação a partir do código"}</span></div>{artifact && <Check className="package-check" size={20} />}</div>
              {platformArtifacts.length > 1 && <label className="architecture-select">Arquitetura<select value={architecture} onChange={(event) => { setArchitecture(event.target.value); setShowInstall(false); }}>{platformArtifacts.map((download) => <option key={download.architecture} value={download.architecture}>{download.architecture === "arm64" ? "ARM64" : "x86_64"}</option>)}</select></label>}
              {artifact ? <>
                <p className="download-description">{platform === "windows" ? "Windows 10/11 e instalador por usuário." : "Binário nativo e instalador local."}<br />Sem precisar instalar Rust.</p>
                <a href={artifact.url} download className="button button-accent download-main-button">Baixar para {platform === "linux" ? "Linux" : "Windows"}<ArrowDownToLine size={18} /></a>
                <div className="download-metadata"><span>{artifact.filename.endsWith(".zip") ? ".zip" : ".tar.gz"} · {artifact.architecture === "arm64" ? "ARM64" : "64 bits"}</span><a href={artifact.checksumUrl} download><ShieldCheck size={13} />Checksum SHA-256<ArrowUpRight size={12} /></a></div>
                <button type="button" className="install-toggle" aria-expanded={showInstall} aria-controls="download-install-instructions" onClick={() => setShowInstall(!showInstall)}>Já baixou? Veja como instalar<ChevronDown className={showInstall ? "is-open" : ""} size={16} /></button>
                {showInstall && <div id="download-install-instructions" className="download-install"><CodeBlock code={installCode} filename={platform === "linux" ? "Terminal" : "PowerShell"} /><p>{platform === "linux" ? "O destino padrão é ~/.local/bin/cppm. Confira se essa pasta está no PATH." : "O instalador registra o PATH do usuário. Abra um novo terminal após instalar."}</p></div>}
              </> : <>
                <p className="download-description">O pacote {platform === "windows" ? "Windows" : "Linux"} ainda não está disponível nesta distribuição. Você pode compilar a CLI com Rust 1.89+.</p>
                <button type="button" className="button button-accent download-main-button" onClick={() => openDoc("instalacao")}>Ver instruções de instalação<ArrowRight size={18} /></button>
                <div className="download-source-note"><Code2 size={14} />Build Tools, CMake, Ninja e Conan são requisitos separados.</div>
              </>}
            </div>
            <div className="download-panel-footer"><span>{platform === "windows" ? "Prepare o ambiente C++" : "Prefere compilar por conta própria?"}</span><button type="button" onClick={() => openDoc(platform === "windows" ? "windows" : "instalacao")}>{platform === "windows" ? "Guia Windows" : "Instalar pelo código"}<ArrowUpRight size={14} /></button></div>
          </div>
        </div>
        <div className="requirements-bar reveal"><span><ShieldCheck size={15} />NO SEU AMBIENTE</span><p>CMake 3.24+<span>/</span>Ninja<span>/</span>GCC, Clang ou MSVC<span>/</span>Conan 2.34+</p><button type="button" onClick={() => openDoc("instalacao")}>Ver requisitos<ArrowUpRight size={14} /></button></div>
      </div>
    </section>
  );
}
