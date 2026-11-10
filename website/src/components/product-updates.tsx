import { ArrowUpRight, Check, Search, ShieldCheck, TerminalSquare } from "lucide-react";
import { CopyButton } from "./ui";

export function ProductUpdates() {
  return <section className="updates-section section-space" id="novidades" aria-labelledby="updates-heading">
    <div className="container">
      <div className="section-heading reveal"><div><span className="eyebrow"><span className="section-index">+</span> MAIS DO CPPM</span><h2 id="updates-heading">Explore com calma.<br /><span className="muted">Distribua com mais cuidado.</span></h2></div><p>Uma interface para descobrir o que usar.<br />Um ofuscador incluído para a versão final.</p></div>
      <div className="updates-grid">
        <article className="update-card reveal">
          <div className="update-card-heading"><TerminalSquare size={21} strokeWidth={1.5} /><span>NO SEU TERMINAL</span><span className="update-tag">INTERATIVO</span></div>
          <h3>Não precisa lembrar de tudo.</h3><p>Pesquise bibliotecas, selecione várias de uma vez e encontre seus scripts e comandos em um só lugar.</p>
          <div className="update-command"><span aria-hidden="true">$</span><code>cppm ui</code><CopyButton text="cppm ui" label="Copiar: cppm ui" /></div>
          <div className="tui-preview" role="img" aria-label="Prévia do terminal: aba Explorar com busca por JSON e bibliotecas selecionadas">
            <div className="tui-preview-top"><strong>◈ cppm</strong><span>~/meu-app</span></div>
            <div className="tui-preview-tabs"><span>Visão geral</span><span className="is-active">Explorar</span><span>Dependências</span><span>Comandos</span></div>
            <div className="tui-preview-search"><Search size={12} /><span>json, formatação, testes…</span><span>/</span></div>
            <div className="tui-preview-row"><span>[✓]</span><strong>fmt</strong><small>Formatação</small></div>
            <div className="tui-preview-row is-selected"><span>[✓]</span><strong>nlohmann_json</strong><small>Serialização</small></div>
            <div className="tui-preview-row"><span>[ ]</span><strong>catch2</strong><small>Testes</small></div>
            <div className="tui-preview-bottom">2 selecionadas <span>Espaço selecionar · Enter instalar</span></div>
          </div>
          <a className="update-doc-link" href="#docs-interativo">Conheça a interface e os atalhos<ArrowUpRight size={15} /></a>
        </article>
        <article className="update-card reveal">
          <div className="update-card-heading"><ShieldCheck size={21} strokeWidth={1.5} /><span>NA VERSÃO FINAL</span><span className="update-tag">LLVM</span></div>
          <h3>Uma camada a mais para seu código.</h3><p>O preset obf prepara o plugin e aplica transformações LLVM em um build separado dos seus Debug e Release.</p>
          <div className="update-command"><span aria-hidden="true">$</span><code>cppm build obf</code><CopyButton text="cppm build obf" label="Copiar: cppm build obf" /></div>
          <div className="protection-preview">
            <span className="protection-preview-label">OBFUSCATOR-LLVM INCLUÍDO</span>
            <div><Check size={14} /><span>Mascaramento de strings elegíveis</span></div>
            <div><Check size={14} /><span>Transformações de fluxo e instruções</span></div>
            <div><Check size={14} /><span>Símbolos e metadados reduzidos</span></div>
            <div className="protection-artifact"><span>saída</span><code>.cppm/build/obf/bin/</code></div>
            <p>Requer Clang + SDK LLVM correspondente. A ofuscação dificulta a análise; não impede toda engenharia reversa.</p>
          </div>
          <a className="update-doc-link" href="#docs-protecao">Veja os requisitos e como testar<ArrowUpRight size={15} /></a>
        </article>
      </div>
    </div>
  </section>;
}
