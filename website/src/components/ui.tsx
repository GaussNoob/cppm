"use client";

import { Check, Copy } from "lucide-react";
import { useEffect, useRef, useState, type CSSProperties } from "react";

export function Brand({ compact = false }: { compact?: boolean }) {
  return (
    <span className={`brand ${compact ? "brand-compact" : ""}`}>
      <svg className="brand-symbol" width="34" height="34" viewBox="0 0 34 34" fill="none" aria-hidden="true">
        <path d="m10.5 8-7 9 7 9m13-18 7 9-7 9M20.5 6l-7 22" stroke="currentColor" strokeWidth="2.6" />
      </svg>
      <span>cppm<span className="brand-dot">.</span></span>
    </span>
  );
}

export function CopyButton({ text, label = "Copiar código", showLabel = false }: { text: string; label?: string; showLabel?: boolean }) {
  const [status, setStatus] = useState<"idle" | "copied" | "error">("idle");
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => () => { if (timer.current) clearTimeout(timer.current); }, []);

  async function copy() {
    try {
      if (navigator.clipboard?.writeText) {
        await navigator.clipboard.writeText(text);
      } else {
        const input = document.createElement("textarea");
        input.value = text;
        input.style.position = "fixed";
        input.style.opacity = "0";
        document.body.append(input);
        input.select();
        const copied = document.execCommand("copy");
        input.remove();
        if (!copied) throw new Error("Clipboard is unavailable");
      }
      setStatus("copied");
    } catch {
      setStatus("error");
    }
    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(() => setStatus("idle"), 2500);
  }

  const feedback = status === "copied" ? "Copiado!" : status === "error" ? "Selecione o texto para copiar" : label;

  return (
    <button type="button" className={`copy-button ${status === "copied" ? "is-copied" : ""}`} onClick={copy} aria-label={feedback} title={feedback}>
      {status === "copied" ? <Check size={15} /> : <Copy size={15} />}
      {showLabel && <span>{status === "idle" ? "Copiar" : feedback}</span>}
      <span className="sr-only" role="status">{status === "idle" ? "" : feedback}</span>
    </button>
  );
}

export function HighlightedCode({ code }: { code: string }) {
  const tokens = code.split(/("[^"\n]*"|'[^'\n]*'|#[^\n]*|\b(?:cppm|cargo|uv|powershell|sh|cd|fish_add_path)\b|--[\w-]+|\[[\w.-]+\])/g);
  return <>{tokens.map((token, i) => {
    const type = token.startsWith("#") ? "comment"
      : /^["']/.test(token) ? "string"
      : token.startsWith("--") ? "flag"
      : token.startsWith("[") ? "section"
      : /^(cppm|cargo|uv|powershell|sh|cd|fish_add_path)$/.test(token) ? "command" : "";
    return type ? <span className={`token-${type}`} key={i}>{token}</span> : token;
  })}</>;
}

export function CodeBlock({ code, filename = "Terminal", numbered = false }: { code: string; filename?: string; numbered?: boolean }) {
  return (
    <div className="code-block">
      <div className="code-block-header"><span><span className="code-file-dot" />{filename}</span><CopyButton text={code} /></div>
      <pre tabIndex={0} aria-label={`Código: ${filename}`}><code>{numbered ? code.split("\n").map((line, i) => <span className="code-line" key={i} style={{ "--line": i } as CSSProperties}><span className="line-number" aria-hidden="true">{i + 1}</span><span><HighlightedCode code={line} />{"\n"}</span></span>) : <HighlightedCode code={code} />}</code></pre>
    </div>
  );
}

export function MotionObserver() {
  useEffect(() => {
    const motion = window.matchMedia("(prefers-reduced-motion: reduce)");
    if (motion.matches) return;
    const elements = document.querySelectorAll(".reveal");
    const observer = new IntersectionObserver((entries) => entries.forEach((entry) => {
      if (entry.isIntersecting) {
        entry.target.classList.add("is-visible");
        observer.unobserve(entry.target);
      }
    }), { threshold: 0.08 });
    document.documentElement.dataset.motion = "ready";
    elements.forEach((element) => observer.observe(element));
    return () => {
      observer.disconnect();
      delete document.documentElement.dataset.motion;
    };
  }, []);
  return null;
}
