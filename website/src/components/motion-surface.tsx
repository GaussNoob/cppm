"use client";

import { useEffect, useRef, type PointerEvent, type ReactNode } from "react";

export function MotionSurface({ children, className = "" }: { children: ReactNode; className?: string }) {
  const surface = useRef<HTMLDivElement>(null);
  const frame = useRef(0);
  const reduced = useRef(true);

  useEffect(() => {
    const media = window.matchMedia("(prefers-reduced-motion: reduce)");
    function update() { reduced.current = media.matches; }
    update();
    media.addEventListener("change", update);
    return () => { cancelAnimationFrame(frame.current); media.removeEventListener("change", update); };
  }, []);

  function move(event: PointerEvent<HTMLDivElement>) {
    if (reduced.current || event.pointerType !== "mouse") return;
    cancelAnimationFrame(frame.current);
    const rect = event.currentTarget.getBoundingClientRect();
    const x = (event.clientX - rect.left) / rect.width;
    const y = (event.clientY - rect.top) / rect.height;
    frame.current = requestAnimationFrame(() => {
      const style = surface.current?.style;
      style?.setProperty("--tilt-x", `${(0.5 - y) * 4}deg`);
      style?.setProperty("--tilt-y", `${(x - 0.5) * 5}deg`);
      style?.setProperty("--spotlight-x", `${x * 100}%`);
      style?.setProperty("--spotlight-y", `${y * 100}%`);
    });
  }

  function reset() {
    cancelAnimationFrame(frame.current);
    surface.current?.style.setProperty("--tilt-x", "0deg");
    surface.current?.style.setProperty("--tilt-y", "0deg");
  }

  return <div ref={surface} className={`motion-surface ${className}`} onPointerMove={move} onPointerLeave={reset}><div className="motion-surface-inner">{children}</div></div>;
}
