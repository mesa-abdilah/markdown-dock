import { useCallback, useEffect, useRef } from "react";
import { defaultDockIpc } from "../../lib/ipc";

interface RenderedViewProps {
  html: string;
  activeMatchIdx?: number;
  findQuery?: string;
}

export function RenderedView({ html, activeMatchIdx = 0, findQuery = "" }: RenderedViewProps) {
  const containerRef = useRef<HTMLDivElement>(null);

  const onClick = useCallback((e: React.MouseEvent<HTMLDivElement>) => {
    const target = e.target as HTMLElement | null;
    const anchor = target?.closest("a");
    if (!anchor) return;
    e.preventDefault();
    const href = anchor.getAttribute("href");
    if (
      href &&
      (href.startsWith("https://") || href.startsWith("http://") || href.startsWith("mailto:"))
    ) {
      void defaultDockIpc.openExternalUrl(href);
    }
  }, []);

  useEffect(() => {
    const root = containerRef.current;
    if (!root) return;
    root.querySelectorAll(".dmd-find-active").forEach((el) => {
      el.classList.remove("dmd-find-active");
    });
    if (!findQuery.trim()) return;
    const active = root.querySelector(`[data-find-idx="${activeMatchIdx}"]`);
    if (active) {
      active.classList.add("dmd-find-active");
      if (typeof active.scrollIntoView === "function") {
        active.scrollIntoView({ block: "center", behavior: "smooth" });
      }
    }
  }, [html, activeMatchIdx, findQuery]);

  return (
    <div
      ref={containerRef}
      data-selectable="true"
      onClick={onClick}
      className="dmd-prose select-text"
      // HTML is produced by comrak (unsafe=false) + sanitized by ammonia in Rust
      dangerouslySetInnerHTML={{ __html: html }}
    />
  );
}
