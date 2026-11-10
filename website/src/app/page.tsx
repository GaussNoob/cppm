import { ArrowDown, ArrowDownToLine, ArrowRight, ArrowUpRight, Box, Braces, Check, Command, FileCode2, Layers2, LockKeyhole, Settings2, Workflow } from "lucide-react";
import { Header } from "@/components/header";
import { TerminalDemo } from "@/components/terminal";
import { MotionSurface } from "@/components/motion-surface";
import { Documentation } from "@/components/documentation";
import { Download } from "@/components/download";
import { ProductUpdates } from "@/components/product-updates";
import { Brand, CodeBlock, CopyButton, MotionObserver } from "@/components/ui";
import project from "@/generated/project.json";

const steps = [
  { number: "01", title: "Comece com o essencial.", text: "Manifesto, CMake, código inicial e um teste. Uma base pequena, pronta para ser sua.", command: "cppm init meu-app", verb: "criar", icon: FileCode2 },
  { number: "02", title: "Traga suas bibliotecas.", text: "O Conan resolve os pacotes. O cppm conecta includes e targets ao seu projeto.", command: "cppm add fmt", verb: "adicionar", icon: Box },
  { number: "03", title: "Dê vida ao seu código.", text: "Compile em Debug, execute e teste. Para distribuir, gere um build em Release.", command: "cppm run dev", verb: "executar", icon: Workflow },
];

export default function Home() {
  return <>
    <a href="#conteudo" className="skip-link">Pular para o conteúdo</a>
    <MotionObserver />
    <Header version={project.version} />
    <main id="conteudo">
      <section className="hero" id="inicio" aria-labelledby="hero-heading">
        <div className="hero-grid container">
          <div className="hero-copy">
            <a href="#download" className="release-link hero-enter"><span className="status-dot done" /><span>v{project.version}</span><span className="release-separator">/</span>Pequeno por escolha.<ArrowUpRight size={13} /></a>
            <h1 id="hero-heading"><span className="hero-line"><span>Menos setup.</span></span><span className="hero-line"><span>Mais C<span className="hero-pluses"><span>+</span><span>+</span></span>.</span></span></h1>
            <p className="hero-description hero-enter">Do primeiro arquivo ao executável.<br />Um jeito direto de criar, gerenciar e<br className="hero-description-break" /> construir seus projetos C++.</p>
            <div className="hero-actions hero-enter"><a href="#download" className="button button-accent">Instalar cppm<ArrowDown size={18} /></a><a href="#documentacao" className="button button-quiet">Ler a documentação<ArrowUpRight size={17} /></a></div>
            <div className="hero-footnote hero-enter"><span><Check size={13} />Open source, licença MIT</span><span className="footnote-divider" /><span>O terminal é a sua casa.</span></div>
          </div>
          <MotionSurface className="hero-visual hero-enter"><TerminalDemo /></MotionSurface>
        </div>
        <div className="hero-bottom container"><a href="#fluxo" className="explore-link"><span className="explore-arrow"><ArrowDown size={13} /></span>Veja como funciona</a><span className="hero-bottom-note">C++20 · Linux e Windows</span></div>
      </section>

      <section className="stack-strip" aria-label="Ferramentas usadas pelo cppm"><div className="container stack-inner"><p className="stack-caption">Por trás<br /><strong>dos comandos.</strong></p><div className="stack-item"><Settings2 size={25} aria-hidden="true" /><span>Rust<small>Executa a CLI</small></span></div><div className="stack-item"><svg width="25" height="27" viewBox="0 0 25 27" fill="none" aria-hidden="true"><path d="m12.5 2-10 22h20L12.5 2Z" stroke="currentColor" strokeWidth="1.4" /><path d="m12.5 2 2 14-12 8m12-8 8 8" stroke="currentColor" strokeWidth="1.4" /></svg><span>CMake<small>Configura o projeto</small></span></div><div className="stack-item"><Command size={25} aria-hidden="true" /><span>Ninja<small>Compila o código</small></span></div><div className="stack-item"><Layers2 size={25} aria-hidden="true" /><span>Conan 2<small>Gerencia as bibliotecas</small></span></div></div></section>

      <section className="workflow-section section-space" id="fluxo" aria-labelledby="workflow-heading">
        <div className="container">
          <div className="section-heading reveal"><div><span className="eyebrow"><span className="section-index">01 /</span> DO ZERO AO EXECUTÁVEL</span><h2 id="workflow-heading">Entre a ideia e o código,<br /><span className="muted">poucos comandos.</span></h2></div><p>Uma CLI para organizar o básico.<br className="desktop-break" /> Espaço de sobra para o que vem depois.</p></div>
          <div className="workflow-steps">{steps.map((step) => <article className="workflow-step reveal" key={step.number}><div className="workflow-step-top"><span className="step-number">{step.number}</span><span className="step-verb">{step.verb}</span><step.icon size={19} strokeWidth={1.5} /></div><h3>{step.title}</h3><p>{step.text}</p><div className="step-command"><span aria-hidden="true">$</span><code>{step.command}</code><CopyButton text={step.command} label={`Copiar: ${step.command}`} /></div></article>)}</div>
          <div className="workflow-bottom reveal"><span><Braces size={17} /> Aplicações nativas, APIs HTTP e WebAssembly.</span><a href="#documentacao">Encontre seu ponto de partida<ArrowRight size={15} /></a></div>
        </div>
      </section>

      <section className="ownership-section" aria-labelledby="ownership-heading">
        <div className="container ownership-grid">
          <div className="ownership-copy reveal"><span className="eyebrow"><span className="section-index">02 /</span> FAMILIAR POR DENTRO</span><h2 id="ownership-heading">Seu código.<br />Suas ferramentas.<br /><span>Suas regras.</span></h2><p>O cppm organiza o caminho. Você continua com o controle do projeto e das ferramentas que já conhece.</p><a className="ownership-link" href="#documentacao">Veja como tudo se conecta<ArrowUpRight size={18} /></a></div>
          <div className="ownership-code reveal"><div className="ownership-code-label"><span><span className="status-dot" />UM ARQUIVO. UMA VISÃO DO PROJETO.</span><span>~/meu-app</span></div><CodeBlock filename="cppm.toml" numbered code={'[package]\nname = "meu-app"\nversion = "0.1.0"\ncpp-standard = 20\n\n[dependencies]\nfmt = "11.0.2"\n\n[scripts]\ndev = ["cppm", "exec", "--"]\nbuild = ["cppm", "build", "--release"]'} /><div className="ownership-code-caption"><FileCode2 size={14} /><span>Legível para você. Direto para o cppm.</span><Check size={14} /></div></div>
        </div>
        <div className="container ownership-details"><div className="reveal"><FileCode2 size={20} strokeWidth={1.5} /><div><h3>CMake continua seu.</h3><p>Adicione targets e opções. A CLI não reescreve seu CMakeLists.txt depois do init.</p></div></div><div className="reveal"><LockKeyhole size={20} strokeWidth={1.5} /><div><h3>Dependências registradas.</h3><p>Versões exatas no manifesto. Receitas e revisões resolvidas no cppm.lock.</p></div></div><div className="reveal"><Layers2 size={20} strokeWidth={1.5} /><div><h3>Cache bem aproveitado.</h3><p>O Conan compartilha os pacotes. O Ninja reutiliza o que não mudou.</p></div></div></div>
      </section>

      <ProductUpdates />
      <Documentation />
      <Download version={project.version} downloads={project.downloads} />
    </main>

    <footer className="site-footer"><div className="container"><div className="footer-top"><div><a href="#inicio" className="brand-link" aria-label="cppm — voltar ao início"><Brand compact /></a><p>Um pequeno gerenciador.<br />Um bom começo para o seu C++.</p></div><div className="footer-links"><a href="#documentacao">Documentação<ArrowUpRight size={13} /></a><a href="/docs/architecture.md" download>Arquitetura<ArrowDownToLine size={13} /></a><a href="/LICENSE.txt" target="_blank" rel="noreferrer">Licença MIT<ArrowUpRight size={13} /></a><a href="/THIRD_PARTY_NOTICES.txt" target="_blank" rel="noreferrer">Componentes de terceiros<ArrowUpRight size={13} /></a></div><a href="#inicio" className="back-to-top" aria-label="Voltar ao topo"><ArrowUpRight size={22} /></a></div><div className="footer-bottom"><span>Feito em Rust. Feito para C++.</span><span className="footer-build"><span className="status-dot done" />cppm v{project.version}<span>© {new Date().getFullYear()}</span></span></div></div></footer>
  </>;
}
