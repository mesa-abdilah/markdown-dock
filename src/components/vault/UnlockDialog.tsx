import { useState } from "react";
import { KeyRound, LockOpen, X } from "lucide-react";
import { useVaultStore } from "../../stores/vaultStore";
import { CustomCheckbox } from "../ui/CustomCheckbox";

export function UnlockDialog() {
  const modalMode = useVaultStore((s) => s.modalMode);
  const busy = useVaultStore((s) => s.busy);
  const error = useVaultStore((s) => s.error);
  const unlockWithPassword = useVaultStore((s) => s.unlockWithPassword);
  const unlockWithRecovery = useVaultStore((s) => s.unlockWithRecovery);
  const changePassword = useVaultStore((s) => s.changePassword);
  const openModal = useVaultStore((s) => s.openModal);

  const [tab, setTab] = useState<"password" | "recovery">("password");
  const [password, setPassword] = useState("");
  const [recoveryKey, setRecoveryKey] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [remember, setRemember] = useState(true);

  const handleUnlockPw = async (e: React.FormEvent) => {
    e.preventDefault();
    const ok = await unlockWithPassword(password, remember);
    if (ok) setPassword("");
  };

  const handleUnlockRecovery = async (e: React.FormEvent) => {
    e.preventDefault();
    const ok = await unlockWithRecovery(recoveryKey, newPassword || null, remember);
    if (ok) {
      setRecoveryKey("");
      setNewPassword("");
    }
  };

  const handleChangePw = async (e: React.FormEvent) => {
    e.preventDefault();
    const ok = await changePassword(password, newPassword);
    if (ok) {
      setPassword("");
      setNewPassword("");
    }
  };

  return (
    <div className="flex h-full flex-col justify-between p-3.5 text-xs">
      <div className="space-y-3 overflow-y-auto">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            {modalMode === "change-password" ? (
              <KeyRound className="h-4 w-4 text-indigo-400" />
            ) : (
              <LockOpen className="h-4 w-4 text-amber-400" />
            )}
            <span className="font-semibold text-zinc-100">
              {modalMode === "change-password" ? "Change Master Password" : "Unlock Vault"}
            </span>
          </div>
          <button
            type="button"
            onClick={() => openModal("none")}
            aria-label="Close vault dialog"
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

        {modalMode === "change-password" ? (
          <form onSubmit={handleChangePw} className="space-y-3">
            <p className="text-[11px] text-zinc-400">
              Re-wraps your Vault Key with a new Argon2id KEK in O(1) without re-encrypting backups.
            </p>
            <label className="block space-y-1">
              <span className="text-[11px] text-zinc-300">Current Password</span>
              <input
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                className="h-8 w-full rounded-lg border border-white/10 bg-white/[0.06] px-2.5 text-xs text-zinc-100 focus:border-indigo-400/60 focus:outline-none"
                required
              />
            </label>
            <label className="block space-y-1">
              <span className="text-[11px] text-zinc-300">New Master Password</span>
              <input
                type="password"
                value={newPassword}
                onChange={(e) => setNewPassword(e.target.value)}
                className="h-8 w-full rounded-lg border border-white/10 bg-white/[0.06] px-2.5 text-xs text-zinc-100 focus:border-indigo-400/60 focus:outline-none"
                required
              />
            </label>
            <button
              type="submit"
              disabled={busy}
              className="w-full rounded-lg bg-indigo-500/90 py-2 text-xs font-semibold text-white hover:bg-indigo-500 disabled:opacity-45"
            >
              {busy ? "Re-wrapping key..." : "Update Password"}
            </button>
          </form>
        ) : (
          <>
            <div className="flex gap-1 rounded-lg bg-white/[0.05] p-1">
              <button
                type="button"
                onClick={() => setTab("password")}
                className={`flex-1 rounded-md py-1 text-[11px] font-medium ${
                  tab === "password" ? "bg-white/15 text-white" : "text-zinc-400 hover:text-zinc-200"
                }`}
              >
                Master Password
              </button>
              <button
                type="button"
                onClick={() => setTab("recovery")}
                className={`flex-1 rounded-md py-1 text-[11px] font-medium ${
                  tab === "recovery" ? "bg-white/15 text-white" : "text-zinc-400 hover:text-zinc-200"
                }`}
              >
                Recovery Key
              </button>
            </div>

            {tab === "password" ? (
              <form onSubmit={handleUnlockPw} className="space-y-3">
                <label className="block space-y-1">
                  <span className="text-[11px] text-zinc-300">Master Password</span>
                  <input
                    type="password"
                    value={password}
                    onChange={(e) => setPassword(e.target.value)}
                    autoFocus
                    className="h-8 w-full rounded-lg border border-white/10 bg-white/[0.06] px-2.5 text-xs text-zinc-100 focus:border-indigo-400/60 focus:outline-none"
                    required
                  />
                </label>
                <label className="flex cursor-pointer items-center justify-between text-[11px] text-zinc-300">
                  <span>Remember key in OS Credential Manager</span>
                  <CustomCheckbox variant="switch" checked={remember} onChange={setRemember} />
                </label>
                <button
                  type="submit"
                  disabled={busy}
                  className="w-full rounded-lg bg-indigo-500/90 py-2 text-xs font-semibold text-white hover:bg-indigo-500 disabled:opacity-45"
                >
                  {busy ? "Unlocking..." : "Unlock Vault"}
                </button>
              </form>
            ) : (
              <form onSubmit={handleUnlockRecovery} className="space-y-3">
                <label className="block space-y-1">
                  <span className="text-[11px] text-zinc-300">52-char Recovery Key</span>
                  <textarea
                    rows={3}
                    value={recoveryKey}
                    onChange={(e) => setRecoveryKey(e.target.value)}
                    placeholder="XXXX-XXXX-..."
                    className="w-full rounded-lg border border-white/10 bg-white/[0.06] p-2 font-mono text-[11px] uppercase text-zinc-100 focus:border-indigo-400/60 focus:outline-none"
                    required
                  />
                </label>
                <label className="block space-y-1">
                  <span className="text-[11px] text-zinc-300">
                    Set New Master Password (optional)
                  </span>
                  <input
                    type="password"
                    value={newPassword}
                    onChange={(e) => setNewPassword(e.target.value)}
                    className="h-8 w-full rounded-lg border border-white/10 bg-white/[0.06] px-2.5 text-xs text-zinc-100 focus:border-indigo-400/60 focus:outline-none"
                  />
                </label>
                <button
                  type="submit"
                  disabled={busy}
                  className="w-full rounded-lg bg-amber-500/85 py-2 text-xs font-semibold text-white hover:bg-amber-500 disabled:opacity-45"
                >
                  {busy ? "Recovering..." : "Unlock with Recovery Key"}
                </button>
              </form>
            )}
          </>
        )}
      </div>
    </div>
  );
}
