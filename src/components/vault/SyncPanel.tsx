import { useState } from "react";
import { Cloud, CloudOff, Download, RefreshCw, UploadCloud } from "lucide-react";
import { useSettingsStore } from "../../stores/settingsStore";
import { useSyncStore } from "../../stores/syncStore";
import { useVaultStore } from "../../stores/vaultStore";
import { ConflictList } from "./ConflictList";

export function SyncPanel() {
  const status = useSyncStore((s) => s.status);
  const busy = useSyncStore((s) => s.busy);
  const lastMessage = useSyncStore((s) => s.lastMessage);
  const error = useSyncStore((s) => s.error);
  const connectDrive = useSyncStore((s) => s.connectDrive);
  const disconnectDrive = useSyncStore((s) => s.disconnectDrive);
  const importRemoteKeyfile = useSyncStore((s) => s.importRemoteKeyfileIfPresent);
  const syncNow = useSyncStore((s) => s.syncNow);
  const pullChanges = useSyncStore((s) => s.pullChanges);
  const openRestoreModal = useSyncStore((s) => s.openModal);

  const vaultState = useVaultStore((s) => s.status.state);
  const settings = useSettingsStore((s) => s.settings);
  const updateSettings = useSettingsStore((s) => s.update);

  const [showOAuthCfg, setShowOAuthCfg] = useState(false);
  const [clientIdDraft, setClientIdDraft] = useState<string | null>(null);
  const [clientSecretDraft, setClientSecretDraft] = useState<string | null>(null);

  const cid = clientIdDraft ?? settings.google_client_id;
  const csec = clientSecretDraft ?? settings.google_client_secret;

  const handleSaveOAuth = async () => {
    await updateSettings({
      google_client_id: cid,
      google_client_secret: csec,
    });
    setClientIdDraft(null);
    setClientSecretDraft(null);
    setShowOAuthCfg(false);
  };

  return (
    <div className="space-y-2.5 border-b border-white/[0.07] pb-3">
      <div className="flex items-center justify-between">
        <span className="font-semibold text-zinc-200">Google Drive Backup & Sync</span>
        <span
          className={`inline-flex items-center gap-1 rounded px-1.5 py-0.5 text-[10px] font-medium ${
            status.connected
              ? "bg-emerald-500/15 text-emerald-300"
              : "bg-white/10 text-zinc-400"
          }`}
        >
          {status.connected ? <Cloud className="h-3 w-3" /> : <CloudOff className="h-3 w-3" />}
          {status.connected ? "Connected" : "Disconnected"}
        </span>
      </div>

      {error && (
        <p className="rounded-lg border border-rose-500/30 bg-rose-500/15 px-2.5 py-1.5 text-[11px] text-rose-200">
          {error}
        </p>
      )}
      {lastMessage && (
        <p className="rounded-lg border border-emerald-500/25 bg-emerald-500/10 px-2.5 py-1.5 text-[11px] text-emerald-200">
          {lastMessage}
        </p>
      )}

      <div className="flex flex-wrap items-center gap-1.5">
        {!status.connected ? (
          <button
            type="button"
            disabled={busy}
            onClick={() => void connectDrive()}
            className="rounded-md bg-indigo-500/85 px-2.5 py-1 text-[11px] font-medium text-white hover:bg-indigo-500 disabled:opacity-50"
          >
            {busy ? "Waiting for OAuth..." : "Connect Google Drive"}
          </button>
        ) : (
          <>
            <button
              type="button"
              disabled={busy || vaultState !== "Unlocked"}
              onClick={() => void syncNow()}
              title="Backup changed local files to Drive (1-way)"
              className="inline-flex items-center gap-1 rounded-md bg-indigo-500/85 px-2.5 py-1 text-[11px] font-medium text-white hover:bg-indigo-500 disabled:opacity-45"
            >
              <UploadCloud className="h-3 w-3" />
              <span>Backup Now</span>
            </button>
            <button
              type="button"
              disabled={busy || vaultState !== "Unlocked"}
              onClick={() => void pullChanges()}
              title="Pull changes from other devices into local folders"
              className="inline-flex items-center gap-1 rounded-md bg-white/[0.08] px-2.5 py-1 text-[11px] text-zinc-200 hover:bg-white/15 disabled:opacity-45"
            >
              <RefreshCw className="h-3 w-3" />
              <span>Sync / Pull</span>
            </button>
            <button
              type="button"
              disabled={busy || vaultState !== "Unlocked"}
              onClick={() => openRestoreModal("restore")}
              className="inline-flex items-center gap-1 rounded-md bg-white/[0.08] px-2.5 py-1 text-[11px] text-zinc-200 hover:bg-white/15 disabled:opacity-45"
            >
              <Download className="h-3 w-3" />
              <span>Restore...</span>
            </button>
            {vaultState === "Uninitialized" && (
              <button
                type="button"
                disabled={busy}
                onClick={() => void importRemoteKeyfile()}
                className="rounded-md bg-amber-500/20 px-2.5 py-1 text-[11px] text-amber-200 hover:bg-amber-500/30"
              >
                Import Existing Vault
              </button>
            )}
            <button
              type="button"
              disabled={busy}
              onClick={() => void disconnectDrive()}
              className="rounded-md bg-rose-500/15 px-2 py-1 text-[11px] text-rose-200 hover:bg-rose-500/25"
            >
              Disconnect
            </button>
          </>
        )}
        <button
          type="button"
          onClick={() => setShowOAuthCfg((v) => !v)}
          className="ml-auto text-[10.5px] text-zinc-400 underline hover:text-zinc-200"
        >
          OAuth Credentials
        </button>
      </div>

      {status.connected && (
        <div className="flex gap-3 text-[10.5px] text-zinc-400">
          <span>Synced: {status.synced}</span>
          <span>Queued: {status.queued}</span>
          <span>Conflicts: {status.conflicts}</span>
        </div>
      )}

      {showOAuthCfg && (
        <div className="space-y-2 rounded-lg border border-white/10 bg-black/25 p-2.5 text-[11px]">
          <input
            type="text"
            value={cid}
            onChange={(e) => setClientIdDraft(e.target.value)}
            placeholder="Google Desktop OAuth Client ID"
            className="h-7 w-full rounded border border-white/10 bg-white/[0.06] px-2 text-zinc-100 focus:outline-none"
          />
          <input
            type="password"
            value={csec}
            onChange={(e) => setClientSecretDraft(e.target.value)}
            placeholder="Google OAuth Client Secret"
            className="h-7 w-full rounded border border-white/10 bg-white/[0.06] px-2 text-zinc-100 focus:outline-none"
          />
          <button
            type="button"
            onClick={() => void handleSaveOAuth()}
            className="rounded bg-indigo-500/80 px-2.5 py-1 text-[11px] font-medium text-white hover:bg-indigo-500"
          >
            Save OAuth Config
          </button>
        </div>
      )}

      <ConflictList />
    </div>
  );
}
