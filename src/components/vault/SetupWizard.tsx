import { useState } from "react";
import { AlertTriangle, Check, Copy, Download, KeyRound, ShieldCheck, X } from "lucide-react";
import type { AeadAlg } from "../../bindings/commands";
import { useVaultStore } from "../../stores/vaultStore";
import { CustomCheckbox } from "../ui/CustomCheckbox";
import { CustomSelect } from "../ui/CustomSelect";

const STRENGTH_LABELS = ["Very weak", "Weak", "Fair", "Strong", "Very strong"];
const STRENGTH_COLORS = [
  "bg-rose-500",
  "bg-orange-500",
  "bg-amber-400",
  "bg-emerald-400",
  "bg-indigo-400",
];

export function SetupWizard() {
  const pendingSetup = useVaultStore((s) => s.pendingSetup);
  const strength = useVaultStore((s) => s.strength);
  const busy = useVaultStore((s) => s.busy);
  const error = useVaultStore((s) => s.error);
  const checkStrength = useVaultStore((s) => s.checkStrength);
  const beginSetup = useVaultStore((s) => s.beginSetup);
  const confirmSetup = useVaultStore((s) => s.confirmSetup);
  const cancelSetup = useVaultStore((s) => s.cancelSetup);

  const [password, setPassword] = useState("");
  const [confirmPw, setConfirmPw] = useState("");
  const [aead, setAead] = useState<AeadAlg>("Aes256Gcm");
  const [remember, setRemember] = useState(true);
  const [copiedKey, setCopiedKey] = useState(false);
  const [savedAck, setSavedAck] = useState(false);
  const [suffixInput, setSuffixInput] = useState("");

  const passwordsMatch = password.length > 0 && password === confirmPw;
  const score = strength?.score ?? 0;
  const cleanedVerifyLen = suffixInput.replace(/[\s-]/g, "").length;
  const canSubmitVerify = cleanedVerifyLen === 6 || cleanedVerifyLen === 52;

  const handleBegin = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!passwordsMatch) return;
    const ok = await beginSetup(password, aead, remember);
    if (ok) {
      // Clear password inputs from React state immediately after keyfile wrapping.
      setPassword("");
      setConfirmPw("");
    }
  };

  const handleCopyRecovery = async () => {
    if (!pendingSetup) return;
    await navigator.clipboard.writeText(pendingSetup.recovery_key);
    setCopiedKey(true);
    window.setTimeout(() => setCopiedKey(false), 1800);
  };

  const handleDownloadRecovery = () => {
    if (!pendingSetup) return;
    const blob = new Blob(
      [
        "Dynamic Markdown Dock — Zero-Knowledge Vault Recovery Key\n",
        "Keep this key offline in a safe place.\n\n",
        `Recovery Key:\n${pendingSetup.recovery_key}\n`,
      ],
      { type: "text/plain;charset=utf-8" },
    );
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "dmd-recovery-key.txt";
    a.click();
    URL.revokeObjectURL(url);
  };

  const handleConfirm = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!savedAck) return;
    await confirmSetup(suffixInput);
  };

  return (
    <div className="flex h-full flex-col justify-between p-3.5 text-xs">
      <div className="space-y-3 overflow-y-auto pr-0.5">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <ShieldCheck className="h-4 w-4 text-indigo-400" />
            <span className="font-semibold text-zinc-100">
              {pendingSetup ? "Save Mandatory Recovery Key" : "Setup Zero-Knowledge Vault"}
            </span>
          </div>
          <button
            type="button"
            onClick={() => void cancelSetup()}
            aria-label="Cancel vault setup"
            className="rounded p-1 text-zinc-400 hover:bg-white/10 hover:text-zinc-100"
          >
            <X className="h-3.5 w-3.5" />
          </button>
        </div>

        {error && (
          <div className="rounded-lg border border-rose-500/35 bg-rose-500/15 px-2.5 py-1.5 text-[11px] text-rose-200">
            {error}
          </div>
        )}

        {!pendingSetup ? (
          <form onSubmit={handleBegin} className="space-y-3">
            <p className="text-[11px] leading-relaxed text-zinc-400">
              Your notes are compressed with <code className="text-zinc-300">zstd</code> and
              encrypted locally before leaving your device. Keys never leave your machine.
            </p>

            <label className="block space-y-1">
              <span className="text-[11px] font-medium text-zinc-300">Master Password</span>
              <input
                type="password"
                value={password}
                onChange={(e) => {
                  const val = e.target.value;
                  setPassword(val);
                  void checkStrength(val);
                }}
                placeholder="Use a strong passphrase (4+ random words)"
                className="h-8 w-full rounded-lg border border-white/10 bg-white/[0.06] px-2.5 text-xs text-zinc-100 focus:border-indigo-400/60 focus:outline-none"
                required
              />
            </label>

            {password.length > 0 && (
              <div className="space-y-1">
                <div className="flex gap-1">
                  {[0, 1, 2, 3].map((idx) => (
                    <div
                      key={idx}
                      className={`h-1 flex-1 rounded-full ${
                        idx <= score - 1 || (score === 0 && idx === 0)
                          ? STRENGTH_COLORS[score]
                          : "bg-white/10"
                      }`}
                    />
                  ))}
                </div>
                <div className="flex justify-between text-[10px]">
                  <span className={score >= 3 ? "text-emerald-300" : "text-amber-300"}>
                    {STRENGTH_LABELS[score]} {score < 3 ? "(min. Strong required)" : ""}
                  </span>
                </div>
                {strength?.warning && (
                  <p className="text-[10px] text-amber-300">{strength.warning}</p>
                )}
              </div>
            )}

            <label className="block space-y-1">
              <span className="text-[11px] font-medium text-zinc-300">Confirm Password</span>
              <input
                type="password"
                value={confirmPw}
                onChange={(e) => setConfirmPw(e.target.value)}
                className="h-8 w-full rounded-lg border border-white/10 bg-white/[0.06] px-2.5 text-xs text-zinc-100 focus:border-indigo-400/60 focus:outline-none"
                required
              />
              {confirmPw.length > 0 && !passwordsMatch && (
                <p className="text-[10px] text-rose-300">Passwords do not match</p>
              )}
            </label>

            <div className="block space-y-1">
              <span className="text-[11px] font-medium text-zinc-300">AEAD Cipher</span>
              <CustomSelect
                ariaLabel="AEAD Cipher"
                value={aead}
                onChange={(val) => setAead(val as AeadAlg)}
                options={[
                  { value: "Aes256Gcm", label: "AES-256-GCM (Hardware AES-NI)" },
                  { value: "ChaCha20Poly1305", label: "ChaCha20-Poly1305" },
                ]}
              />
            </div>

            <label className="flex cursor-pointer items-center justify-between text-[11px] text-zinc-300">
              <span>Remember key in OS Credential Manager</span>
              <CustomCheckbox variant="switch" checked={remember} onChange={setRemember} />
            </label>

            <button
              type="submit"
              disabled={busy || !passwordsMatch || score < 3}
              className="w-full rounded-lg bg-indigo-500/90 py-2 text-xs font-semibold text-white transition-colors hover:bg-indigo-500 disabled:cursor-not-allowed disabled:opacity-45"
            >
              {busy ? "Deriving Argon2id key..." : "Generate Vault & Recovery Key"}
            </button>
          </form>
        ) : (
          <form onSubmit={handleConfirm} className="space-y-3">
            <div className="flex items-start gap-2 rounded-lg border border-amber-400/30 bg-amber-500/10 p-2.5 text-[11px] text-amber-200">
              <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0 text-amber-300" />
              <span>
                <strong>Mandatory:</strong> If you forget your Master Password, this Recovery Key is
                the <strong>only</strong> way to decrypt your backups.
              </span>
            </div>

            <div className="rounded-xl border border-white/10 bg-black/35 p-2.5">
              <div className="mb-1.5 flex items-center justify-between">
                <span className="inline-flex items-center gap-1 text-[10px] font-medium text-zinc-400">
                  <KeyRound className="h-3 w-3" /> 256-bit Recovery Key
                </span>
                <div className="flex gap-1">
                  <button
                    type="button"
                    onClick={() => void handleCopyRecovery()}
                    className="inline-flex items-center gap-1 rounded bg-white/10 px-1.5 py-0.5 text-[10px] text-zinc-200 hover:bg-white/20"
                  >
                    {copiedKey ? <Check className="h-2.5 w-2.5" /> : <Copy className="h-2.5 w-2.5" />}
                    {copiedKey ? "Copied" : "Copy"}
                  </button>
                  <button
                    type="button"
                    onClick={handleDownloadRecovery}
                    className="inline-flex items-center gap-1 rounded bg-white/10 px-1.5 py-0.5 text-[10px] text-zinc-200 hover:bg-white/20"
                  >
                    <Download className="h-2.5 w-2.5" /> Save .txt
                  </button>
                </div>
              </div>
              <p
                data-selectable="true"
                className="select-text font-mono text-[11px] leading-relaxed text-indigo-200"
              >
                {pendingSetup.recovery_key.split("-").map((chunk, idx, arr) => (
                  <span key={idx} className="inline-block whitespace-nowrap">
                    {chunk}
                    {idx < arr.length - 1 ? "-" : ""}
                  </span>
                ))}
              </p>
            </div>

            <label className="flex cursor-pointer items-center gap-2 text-[11px] text-zinc-300">
              <CustomCheckbox checked={savedAck} onChange={setSavedAck} />
              <span>I have stored this Recovery Key safely offline</span>
            </label>

            <label className="block space-y-1">
              <div className="flex items-center justify-between">
                <span className="text-[11px] font-medium text-zinc-300">
                  Verify last 6 chars (or paste full Recovery Key)
                </span>
                <span className="font-mono text-[10px] text-zinc-400">
                  {cleanedVerifyLen === 52 ? "Full key (52)" : `${cleanedVerifyLen}/6`}
                </span>
              </div>
              <input
                type="text"
                value={suffixInput}
                onChange={(e) => setSuffixInput(e.target.value)}
                placeholder="Last 6 chars (e.g. 12CD34) or paste full key"
                className="h-8 w-full rounded-lg border border-white/10 bg-white/[0.06] px-2.5 font-mono text-xs uppercase text-zinc-100 focus:border-indigo-400/60 focus:outline-none"
                required
              />
              <span className="block text-[10px] text-zinc-400">
                Enter the last 6 letters/digits of the key above (no hyphens), or click Copy and
                paste the full key.
              </span>
            </label>

            <button
              type="submit"
              disabled={busy || !savedAck || !canSubmitVerify}
              className="w-full rounded-lg bg-emerald-500/85 py-2 text-xs font-semibold text-white transition-colors hover:bg-emerald-500 disabled:cursor-not-allowed disabled:opacity-45"
            >
              {busy ? "Activating Vault..." : "Verify & Activate Vault"}
            </button>
          </form>
        )}
      </div>
    </div>
  );
}
