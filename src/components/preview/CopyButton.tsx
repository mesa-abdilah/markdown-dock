import { Check, Copy } from "lucide-react";

interface CopyButtonProps {
  copied: boolean;
  label?: string;
  onClick: () => void;
}

export function CopyButton({ copied, label = "Copy", onClick }: CopyButtonProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      title="Copy Markdown to clipboard (Ctrl+C)"
      className={`inline-flex items-center gap-1 rounded-lg px-2 py-1 text-[11px] font-medium transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-indigo-400/70 ${
        copied
          ? "bg-emerald-500/20 text-emerald-300 ring-1 ring-emerald-400/35"
          : "bg-white/[0.08] text-zinc-200 hover:bg-white/[0.14]"
      }`}
    >
      {copied ? <Check className="h-3 w-3" /> : <Copy className="h-3 w-3" />}
      <span>{copied ? "Copied" : label}</span>
    </button>
  );
}
