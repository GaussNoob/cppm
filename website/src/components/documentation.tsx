"use client";

import { ArrowDownToLine, ArrowRight, BookOpen, ChevronRight, Info, Search, X } from "lucide-react";
import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { docGroups, docs, docSearchText, type DocId, type DocSection } from "@/lib/docs";
import { libraryCatalog } from "@/lib/catalog";
import { LibraryCatalog } from "./catalog";
import { CodeBlock } from "./ui";
import project from "@/generated/project.json";

function SectionContent({ section, topic }: { section: DocSection; topic: DocId }) {
  return <section className="doc-section" id={`doc-${topic}-${section.id}`}>
    <h4>{section.title}</h4>
    {section.paragraphs?.map((paragraph) => <p key={paragraph}>{paragraph}</p>)}
    {section.code && <CodeBlock code={section.code} filename={section.filename ?? "Terminal"} />}
    {section.table && <div className="doc-table-scroll" tabIndex={0} role="region" aria-label={section.title}><table className="doc-table"><thead><tr>{section.table.columns.map((column) => <th key={column}>{column}</th>)}</tr></thead><tbody>{section.table.rows.map((row, index) => <tr key={index}>{row.map((cell, cellIndex) => <td key={cellIndex}>{cell}</td>)}</tr>)}</tbody></table></div>}
  </section>;
}

export function Documentation() {
  const [active, setActive] = useState<DocId>("inicio");
  const [filter, setFilter] = useState("");
  const tabRefs = useRef<Partial<Record<DocId, HTMLButtonElement | null>>>({});
  const topic = docs.find((doc) => doc.id === active) ?? docs[0];
  const related = docs.find((doc) => doc.id === topic.related) ?? docs[0];
  const query = filter.normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase().trim();
  const visibleDocs = docGroups.flatMap((group) => docs.filter((doc) => doc.category === group && docSearchText(doc).includes(query)));

  function select(id: DocId, updateHash = true) {
    setActive(id);
    if (updateHash) window.history.replaceState(null, "", `#docs-${id}`);
  }

  useEffect(() => {
    function fromHash() {
      const match = window.location.hash.match(/^#docs-([\w-]+)(?:\/([\w-]+))?$/);
      if (!match || !docs.some((doc) => doc.id === match[1])) return;
      select(match[1] as DocId, false);
      setFilter("");
      requestAnimationFrame(() => requestAnimationFrame(() => {
        document.getElementById(match[2] ? `doc-${match[1]}-${match[2]}` : "documentacao")?.scrollIntoView({ block: "start" });
      }));
    }
    function onOpen(event: Event) {
      const id = (event as CustomEvent<DocId>).detail;
      if (docs.some((doc) => doc.id === id)) { setFilter(""); select(id); }
    }
    fromHash();
    window.addEventListener("hashchange", fromHash);
    window.addEventListener("cppm:open-doc", onOpen);
    return () => {
      window.removeEventListener("hashchange", fromHash);
      window.removeEventListener("cppm:open-doc", onOpen);
    };
  }, []);

  function keyboardSelect(event: KeyboardEvent, id: DocId) {
    if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) return;
    event.preventDefault();
    const index = visibleDocs.findIndex((doc) => doc.id === id);
    const next = event.key === "Home" ? 0 : event.key === "End" ? visibleDocs.length - 1 : (index + (event.key === "ArrowDown" ? 1 : -1) + visibleDocs.length) % visibleDocs.length;
    select(visibleDocs[next].id);
    tabRefs.current[visibleDocs[next].id]?.focus();
  }

  return <section className="documentation-section section-space" id="documentacao" aria-labelledby="docs-heading">
    <div className="container">
      <div className="section-heading reveal"><div><span className="eyebrow"><span className="section-index">03 /</span> DOCUMENTAÇÃO</span><h2 id="docs-heading">Do primeiro build<br /><span className="muted">ao seu próximo projeto.</span></h2></div><p>Guias, exemplos e referência.<br />Organizados para encontrar e usar.</p></div>
      <div className="docs-workspace reveal">
        <aside className="docs-sidebar">
          <div className="docs-sidebar-inner">
            <div className="docs-sidebar-title"><BookOpen size={16} /><span>Manual do cppm</span><span>v{project.version}</span></div>
            <div className="docs-filter"><Search size={15} /><input aria-label="Filtrar tópicos da documentação" placeholder="Encontrar um tópico..." value={filter} onChange={(event) => setFilter(event.target.value)} />{filter && <button type="button" onClick={() => setFilter("")} aria-label="Limpar filtro de tópicos"><X size={14} /></button>}</div>
            <div className="docs-tabs" role="tablist" aria-label="Tópicos da documentação" aria-orientation="vertical">
              {docGroups.map((group) => {
                const topics = visibleDocs.filter((doc) => doc.category === group);
                return topics.length > 0 && <div className="docs-topic-group" role="presentation" key={group}><span className="docs-group-label">{group}</span>{topics.map((doc) => <button key={doc.id} ref={(node) => { tabRefs.current[doc.id] = node; }} type="button" role="tab" id={`doc-tab-${doc.id}`} aria-selected={active === doc.id} aria-controls="doc-panel" tabIndex={active === doc.id || !visibleDocs.some((entry) => entry.id === active) && visibleDocs[0]?.id === doc.id ? 0 : -1} onClick={() => select(doc.id)} onKeyDown={(event) => keyboardSelect(event, doc.id)}><span>{doc.title}</span><ChevronRight size={13} /></button>)}</div>;
              })}
              {visibleDocs.length === 0 && <p className="docs-filter-empty">Nenhum tópico encontrado.<button type="button" onClick={() => setFilter("")}>Limpar busca</button></p>}
            </div>
            <label className="docs-mobile-select"><span>Explorar a documentação</span><select value={active} onChange={(event) => select(event.target.value as DocId)}>{docGroups.map((group) => <optgroup label={group} key={group}>{docs.filter((doc) => doc.category === group).map((doc) => <option value={doc.id} key={doc.id}>{doc.title}</option>)}</optgroup>)}</select></label>
            <div className="docs-sidebar-bottom"><span>{docs.length} tópicos · {libraryCatalog.length} pacotes</span><a href="/docs/guide.md" download>Baixar guia em Markdown<ArrowDownToLine size={13} /></a></div>
          </div>
        </aside>
        <div className="doc-content" id="doc-panel" role="tabpanel" aria-labelledby="doc-topic-title" tabIndex={0}>
          <div className="doc-breadcrumb"><span>Manual</span><ChevronRight size={12} /><span>{topic.category}</span><ChevronRight size={12} /><span>{topic.title}</span></div>
          <article className="doc-article" key={active}>
            <span className="doc-label">{topic.label}</span>
            <h3 id="doc-topic-title">{topic.title}</h3>
            <p className="doc-description">{topic.description}</p>
            {topic.sections && <nav className="doc-outline" aria-label="Nesta página"><span>Nesta página</span>{topic.sections.map((section) => <a key={section.id} href={`#docs-${topic.id}/${section.id}`}>{section.title}<ArrowRight size={12} /></a>)}</nav>}
            <CodeBlock code={topic.code} filename={topic.filename} numbered={active === "manifesto"} />
            <div className="doc-paragraphs">{topic.paragraphs.map((paragraph) => <p key={paragraph}>{paragraph}</p>)}</div>
            {topic.extraCode && <CodeBlock code={topic.extraCode} filename={topic.extraFilename ?? "Terminal"} />}
            {(topic.fullCatalog || topic.catalogGroups) && <LibraryCatalog groups={topic.catalogGroups} />}
            {topic.sections?.map((section) => <SectionContent key={section.id} section={section} topic={active} />)}
            <div className="doc-note"><Info size={17} /><p>{topic.note}</p></div>
          </article>
          <div className="doc-footer"><a href={topic.reference ?? (active === "instalacao" ? "/docs/installation.md" : "/docs/guide.md")} download>Baixar referência completa<ArrowDownToLine size={14} /></a><button type="button" onClick={() => { select(related.id); document.querySelector(".docs-workspace")?.scrollIntoView({ behavior: "instant", block: "start" }); }}>Próximo: {related.title}<ArrowRight size={15} /></button></div>
        </div>
      </div>
    </div>
  </section>;
}
