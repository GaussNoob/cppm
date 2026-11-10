"use client";

import { ArrowUpRight, Check, Code2, Globe2, Play, RotateCcw, Terminal as TerminalIcon } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { CopyButton, HighlightedCode } from "./ui";

const templates = [
  {
    id: "app", label: "App C++", icon: Code2, folder: "meu-app",
    command: "cppm init meu-app\ncd meu-app\ncppm add fmt\ncppm run dev",
    lines: [
      { command: "cppm init meu-app", output: "Created meu-app in ./meu-app" },
      { command: "cd meu-app", output: null },
      { command: "cppm add fmt", output: "Added fmt" },
      { command: "cppm run dev", output: "Hello, meu-app!" },
    ],
    result: "Seu projeto está rodando.", badge: "C++20",
  },
  {
    id: "api", label: "API HTTP", icon: Globe2, folder: "minha-api",
    command: "cppm init minha-api --template drogon\ncd minha-api\ncppm run dev",
    lines: [
      { command: "cppm init minha-api --template drogon", output: "Created minha-api in ./minha-api" },
      { command: "cd minha-api", output: null },
      { command: "cppm run dev", output: "http://127.0.0.1:8080" },
    ],
    result: "Sua API está no ar.", badge: "DROGON",
  },
  {
    id: "wasm", label: "WebAssembly", icon: Play, folder: "meu-wasm",
    command: "cppm init meu-wasm --template wasm\ncd meu-wasm\ncppm run dev",
    lines: [
      { command: "cppm init meu-wasm --template wasm", output: "Created meu-wasm in ./meu-wasm" },
      { command: "cd meu-wasm", output: null },
      { command: "cppm run dev", output: "HTML + JS + WASM → localhost" },
    ],
    result: "C++ também vive no navegador.", badge: "EMSCRIPTEN",
  },
] as const;

export function TerminalDemo() {
  const [active, setActive] = useState(0);
  const [visible, setVisible] = useState(1);
  const [cycle, setCycle] = useState(0);
  const tabs = useRef<(HTMLButtonElement | null)[]>([]);
  const template = templates[active];
  const complete = visible > template.lines.length;

  useEffect(() => {
    const motion = window.matchMedia("(prefers-reduced-motion: reduce)");
    let timers: ReturnType<typeof setTimeout>[] = [];
    function play() {
      timers.forEach(clearTimeout);
      if (motion.matches) {
        setVisible(template.lines.length + 1);
        return;
      }
      setVisible(1);
      timers = Array.from({ length: template.lines.length }, (_, i) => setTimeout(() => setVisible(i + 2), 650 * (i + 1)));
    }
    play();
    motion.addEventListener("change", play);
    return () => { timers.forEach(clearTimeout); motion.removeEventListener("change", play); };
  }, [active, cycle, template.lines.length]);

  function select(index: number) {
    if (index === active) return;
    setVisible(1);
    setActive(index);
  }

  return (
    <div className="terminal-scene">
      <div className="terminal-coordinate" aria-hidden="true"><span>01 — QUICK START</span><span>~/workspace</span></div>
      <div className="terminal-window">
        <div className="terminal-titlebar">
          <div className="window-dots" aria-hidden="true"><i /><i /><i /></div>
          <span><TerminalIcon size={13} /> {template.folder}</span>
          <CopyButton text={template.command} label="Copiar comandos do exemplo" />
        </div>
        <div className="terminal-tabs" role="tablist" aria-label="Exemplo de projeto">
          {templates.map((item, i) => <button type="button" key={item.id} id={`terminal-tab-${item.id}`} role="tab" aria-selected={active === i} aria-controls="terminal-panel" tabIndex={active === i ? 0 : -1} ref={(node) => { tabs.current[i] = node; }} onClick={() => select(i)} onKeyDown={(event) => {
            if (!["ArrowRight", "ArrowLeft", "Home", "End"].includes(event.key)) return;
            event.preventDefault();
            const next = event.key === "Home" ? 0 : event.key === "End" ? templates.length - 1 : (active + (event.key === "ArrowRight" ? 1 : -1) + templates.length) % templates.length;
            select(next); tabs.current[next]?.focus();
          }}><item.icon size={14} />{item.label}</button>)}
        </div>
        <div className="terminal-body" id="terminal-panel" role="tabpanel" aria-labelledby={`terminal-tab-${template.id}`} tabIndex={0}>
          <span className="terminal-comment"># um novo projeto começa aqui</span>
          <div className="terminal-lines" key={`${active}-${cycle}`}>
            {template.lines.map((line, i) => <div className={`terminal-step ${visible > i ? "is-shown" : ""}`} key={line.command} aria-hidden={visible <= i}>
              <div className="terminal-command"><span className="prompt" aria-hidden="true">❯</span><code><HighlightedCode code={line.command} /></code></div>
              {line.output && <div className={`terminal-output ${i === template.lines.length - 1 ? "terminal-output-final" : ""}`}><span aria-hidden="true">{i === template.lines.length - 1 ? "↳" : "✓"}</span>{line.output}</div>}
            </div>)}
          </div>
          <div className={`terminal-cursor-line ${complete ? "is-shown" : ""}`} aria-hidden="true"><span className="prompt">❯</span><span className="terminal-cursor" /></div>
          <p className="sr-only">Exemplo ilustrativo. Os comandos podem ser copiados usando o botão acima.{active === 2 ? " Ative o SDK Emscripten antes de executar." : ""}</p>
        </div>
        <div className={`terminal-status ${complete ? "is-complete" : "is-running"}`}><span role="status"><span className={`status-dot ${complete ? "done" : ""}`} />{complete ? "Pronto para construir" : "Executando o exemplo"}</span><button type="button" aria-label="Repetir animação do terminal" onClick={() => { setVisible(1); setCycle((value) => value + 1); }}><RotateCcw size={13} /></button><span className="terminal-progress" aria-hidden="true" style={{ transform: `scaleX(${visible / (template.lines.length + 1)})` }} /></div>
      </div>
      <div className={`terminal-result ${complete ? "is-shown" : ""}`}><span className="terminal-result-icon"><Check size={17} /></span><div><span>{template.result}</span><small>{active === 2 ? "Com o SDK Emscripten ativo." : "Agora, a melhor parte: escrever código."}</small></div><span className="terminal-result-badge">{template.badge}<ArrowUpRight size={12} /></span></div>
      <div className="scene-cross scene-cross-top" aria-hidden="true">+</div><div className="scene-cross scene-cross-bottom" aria-hidden="true">+</div>
    </div>
  );
}
