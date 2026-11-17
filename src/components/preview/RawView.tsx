import { useEffect, useRef } from "react";
import type { TextMatchOffset } from "../../lib/findHighlight";

interface RawViewProps {
  raw: string;
  readOnly?: boolean;
  onChange?: (next: string) => void;
  onSaveNow?: () => void;
  onUndo?: () => void;
  onRedo?: () => void;
  matchOffsets?: TextMatchOffset[];
  activeMatchIdx?: number;
  jumpSeq?: number;
}

export function RawView({
  raw,
  readOnly = false,
  onChange,
  onSaveNow,
  onUndo,
  onRedo,
  matchOffsets = [],
  activeMatchIdx = 0,
  jumpSeq = 0,
}: RawViewProps) {
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    if (jumpSeq === 0 || matchOffsets.length === 0) return;
    const el = textareaRef.current;
    const match = matchOffsets[activeMatchIdx];
    if (!el || !match) return;
    el.focus();
    el.setSelectionRange(match.start, match.end);
  }, [jumpSeq, activeMatchIdx, matchOffsets]);

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    const mod = e.ctrlKey || e.metaKey;
    const keyLower = e.key.toLowerCase();

    if (mod && keyLower === "s") {
      e.preventDefault();
      onSaveNow?.();
      return;
    }

    if (mod && !readOnly && keyLower === "z") {
      e.preventDefault();
      if (e.shiftKey) {
        onRedo?.();
      } else {
        onUndo?.();
      }
      return;
    }

    if (mod && !readOnly && keyLower === "y") {
      e.preventDefault();
      onRedo?.();
      return;
    }

    if (e.key === "Tab" && !readOnly && onChange) {
      e.preventDefault();
      e.stopPropagation();
      const el = e.currentTarget;
      const start = el.selectionStart;
      const end = el.selectionEnd;
      const next = `${raw.slice(0, start)}  ${raw.slice(end)}`;
      onChange(next);
      window.requestAnimationFrame(() => {
        el.selectionStart = start + 2;
        el.selectionEnd = start + 2;
      });
    }
  };

  return (
    <textarea
      ref={textareaRef}
      data-selectable="true"
      value={raw}
      readOnly={readOnly}
      onChange={(e) => onChange?.(e.target.value)}
      onKeyDown={handleKeyDown}
      spellCheck={false}
      aria-label="Raw Markdown editor"
      placeholder="Write or paste Markdown here..."
      className="h-full min-h-[320px] w-full resize-none bg-transparent font-mono text-[12px] leading-relaxed text-zinc-200 placeholder-zinc-600 focus:outline-none"
    />
  );
}
