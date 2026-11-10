"use client";

import { ArrowDown, ArrowUpRight, FileText, Menu, Search, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { docs, docSearchText, openDoc } from "@/lib/docs";
import { Brand } from "./ui";

function SearchDialog() {
  const dialog = useRef<HTMLDialogElement>(null);
  const input = useRef<HTMLInputElement>(null);
  const [query, setQuery] = useState("");
  const normalized = query.normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase().trim();
  const results = docs.filter((doc) => docSearchText(doc).includes(normalized));

  function open() {
    setQuery("");
    dialog.current?.showModal();
    input.current?.focus();
  }

  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        if (dialog.current?.open) dialog.current.close();
        else open();
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  return <>
    <button className="search-trigger" type="button" onClick={open} aria-label="Buscar na documentação"><Search size={18} /></button>
    <dialog className="search-dialog" ref={dialog} aria-label="Buscar na documentação" onClick={(event) => { if (event.target === event.currentTarget) dialog.current?.close(); }}>
      <div className="search-dialog-inner">
        <div className="search-input-wrap"><Search size={21} /><input ref={input} value={query} onChange={(event) => setQuery(event.target.value)} placeholder="O que você quer construir?" aria-label="Buscar um tópico ou comando" /><button type="button" onClick={() => dialog.current?.close()} aria-label="Fechar busca"><X size={18} /></button></div>
        <div className="search-dialog-label">{query ? `${results.length} ${results.length === 1 ? "resultado" : "resultados"}` : "EXPLORE A DOCUMENTAÇÃO"}</div>
        <div className="search-results">
          {results.length ? results.map((doc) => <button className="search-result" type="button" key={doc.id} onClick={() => { dialog.current?.close(); openDoc(doc.id); }}><FileText size={18} /><span><strong>{doc.title}</strong><small>{doc.short}</small></span><ArrowUpRight size={17} /></button>) : <div className="search-empty"><Search size={25} /><p>Nenhum tópico encontrado.</p><span>Tente “instalação”, “fmt” ou “build”.</span></div>}
        </div>
        <div className="search-dialog-footer"><span>Documentação do cppm</span><span><kbd>esc</kbd> para fechar</span></div>
      </div>
    </dialog>
  </>;
}

export function Header({ version }: { version: string }) {
  const [menuOpen, setMenuOpen] = useState(false);
  const [scrolled, setScrolled] = useState(false);
  const [activeSection, setActiveSection] = useState("inicio");
  const header = useRef<HTMLElement>(null);

  useEffect(() => {
    let frame = 0;
    function update() {
      setScrolled(window.scrollY > 32);
      frame = 0;
    }
    function onScroll() { if (!frame) frame = requestAnimationFrame(update); }
    update();
    window.addEventListener("scroll", onScroll, { passive: true });
    const observer = new IntersectionObserver((entries) => {
      entries.forEach((entry) => { if (entry.isIntersecting) setActiveSection(entry.target.id); });
    }, { rootMargin: "-12% 0px -60% 0px", threshold: 0 });
    ["inicio", "fluxo", "documentacao", "download"].forEach((id) => { const element = document.getElementById(id); if (element) observer.observe(element); });
    return () => { window.removeEventListener("scroll", onScroll); cancelAnimationFrame(frame); observer.disconnect(); };
  }, []);

  useEffect(() => {
    if (!menuOpen) return;
    function onKey(event: KeyboardEvent) { if (event.key === "Escape") setMenuOpen(false); }
    function onOutside(event: PointerEvent) { if (event.target instanceof Node && !header.current?.contains(event.target)) setMenuOpen(false); }
    window.addEventListener("keydown", onKey);
    window.addEventListener("pointerdown", onOutside);
    return () => { window.removeEventListener("keydown", onKey); window.removeEventListener("pointerdown", onOutside); };
  }, [menuOpen]);

  return (
    <header ref={header} className={`site-header ${scrolled ? "is-scrolled" : ""} ${menuOpen ? "menu-is-open" : ""}`}>
      <div className="header-inner">
        <a className="brand-link" href="#inicio" aria-label="cppm — início" onClick={() => setMenuOpen(false)}><Brand /></a>
        <nav className="desktop-nav" aria-label="Navegação principal">{[{ id: "inicio", label: "Início" }, { id: "fluxo", label: "Como funciona" }, { id: "documentacao", label: "Documentação" }].map((item) => <a href={`#${item.id}`} key={item.id} aria-current={activeSection === item.id ? "location" : undefined}>{item.label}</a>)}</nav>
        <div className="header-actions"><SearchDialog /><a className="header-download" href="#download" onClick={() => setMenuOpen(false)}><span className="download-label-desktop">Instalar cppm</span><span className="download-label-mobile">Instalar</span><ArrowDown size={16} /></a><button type="button" className="mobile-menu-button" onClick={() => setMenuOpen(!menuOpen)} aria-expanded={menuOpen} aria-controls="mobile-nav" aria-label={menuOpen ? "Fechar menu" : "Abrir menu"}>{menuOpen ? <X size={20} /> : <Menu size={20} />}</button></div>
      </div>
      {menuOpen && <nav id="mobile-nav" className="mobile-nav" aria-label="Navegação mobile"><a href="#inicio" onClick={() => setMenuOpen(false)}>Início<ArrowUpRight size={18} /></a><a href="#fluxo" onClick={() => setMenuOpen(false)}>Como funciona<ArrowUpRight size={18} /></a><a href="#documentacao" onClick={() => setMenuOpen(false)}>Documentação<ArrowUpRight size={18} /></a><div className="mobile-nav-bottom"><span className="status-dot done" />Open source · MIT<span>v{version}</span></div></nav>}
    </header>
  );
}
