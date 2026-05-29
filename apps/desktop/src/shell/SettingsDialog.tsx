import { Button, Input, Modal } from "@heroui/react";
import {
  Database,
  FolderOpen,
  LogOut,
  Settings,
  Shield,
  Trash2,
  User,
  Wifi,
} from "lucide-react";
import { type ReactNode, useEffect, useState } from "react";
import { useLocale } from "../hooks/useLocale";
import { notifySettingsChanged } from "../hooks/useTheme";
import { dbCacheStats, dbClearCache, openDataDir, type CacheSizeInfo } from "../lib/teamai";

type SettingsSection = "general" | "network" | "cache" | "account" | "about";

const sectionDefs: Array<{ id: SettingsSection; labelKey: string; icon: ReactNode }> = [
  { id: "general", labelKey: "settings.general", icon: <Settings size={15} /> },
  { id: "network", labelKey: "settings.network", icon: <Wifi size={15} /> },
  { id: "cache", labelKey: "settings.cache", icon: <Database size={15} /> },
  { id: "account", labelKey: "settings.account", icon: <User size={15} /> },
  { id: "about", labelKey: "settings.about", icon: <Shield size={15} /> },
];

export interface AppSettings {
  theme: "system" | "light" | "dark";
  accentColor: string;
  language: "auto" | "zh-CN" | "en";
  proxyMode: "none" | "system" | "custom";
  proxyUrl: string;
  requestTimeout: number;
}

const defaultSettings: AppSettings = {
  theme: "system",
  accentColor: "blue",
  language: "auto",
  proxyMode: "none",
  proxyUrl: "",
  requestTimeout: 30,
};

function loadSettings(): AppSettings {
  try {
    const raw = localStorage.getItem("teamai-settings");
    if (raw) return { ...defaultSettings, ...JSON.parse(raw) };
  } catch { /* ignore */ }
  return defaultSettings;
}

function saveSettings(settings: AppSettings) {
  localStorage.setItem("teamai-settings", JSON.stringify(settings));
  notifySettingsChanged();
}

export function SettingsDialog({
  open,
  onOpenChange,
  authLogin,
  authScopes,
  onLogout,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  authLogin: string | null | undefined;
  authScopes: string[];
  onLogout: () => void;
}) {
  const { t } = useLocale();
  const [section, setSection] = useState<SettingsSection>("general");
  const [settings, setSettings] = useState<AppSettings>(loadSettings);

  const update = <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => {
    const next = { ...settings, [key]: value };
    setSettings(next);
    saveSettings(next);
  };

  return (
    <Modal isOpen={open} onOpenChange={onOpenChange}>
      <Modal.Backdrop>
        <Modal.Container size="lg">
          <Modal.Dialog className="mx-auto rounded-[12px] bg-[var(--bg-elevated)] outline-none" style={{ width: 600, maxWidth: 600 }}>
            <Modal.Header className="border-b border-[var(--line)] px-5 py-4">
              <Modal.Heading className="text-[15px] font-semibold">{t("settings.title")}</Modal.Heading>
            </Modal.Header>
            <Modal.Body className="p-0">
              <div className="grid min-h-[420px] grid-cols-[140px_1fr] divide-x divide-[var(--line)]">
                {/* Left nav */}
                <nav className="flex flex-col gap-0.5 p-3">
                  {sectionDefs.map((s) => (
                    <button
                      key={s.id}
                      type="button"
                      onClick={() => setSection(s.id)}
                      className={`settings-nav-item ${section === s.id ? "is-active" : ""}`}
                    >
                      {s.icon}
                      <span>{t(s.labelKey)}</span>
                    </button>
                  ))}
                </nav>

                {/* Right content */}
                <div className="scroll-area p-5">
                  {section === "general" ? (
                    <GeneralSection settings={settings} update={update} />
                  ) : section === "network" ? (
                    <NetworkSection settings={settings} update={update} />
                  ) : section === "cache" ? (
                    <CacheSection />
                  ) : section === "account" ? (
                    <AccountSection authLogin={authLogin} authScopes={authScopes} onLogout={onLogout} />
                  ) : (
                    <AboutSection />
                  )}
                </div>
              </div>
            </Modal.Body>
          </Modal.Dialog>
        </Modal.Container>
      </Modal.Backdrop>
    </Modal>
  );
}

function SettingsRow({ label, description, children }: { label: string; description?: string; children: ReactNode }) {
  return (
    <div className="settings-row">
      <div className="min-w-0 flex-1">
        <div className="text-[13px] font-medium text-[var(--fg)]">{label}</div>
        {description ? <div className="mt-0.5 text-[11.5px] text-[var(--fg-muted)]">{description}</div> : null}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

function SelectControl({ value, options, onChange }: { value: string; options: Array<{ value: string; label: string }>; onChange: (v: string) => void }) {
  return (
    <select
      value={value}
      onChange={(e) => onChange(e.target.value)}
      className="settings-select"
    >
      {options.map((opt) => (
        <option key={opt.value} value={opt.value}>{opt.label}</option>
      ))}
    </select>
  );
}

function ToggleControl({ checked, onChange }: { checked: boolean; onChange: (v: boolean) => void }) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      onClick={() => onChange(!checked)}
      className={`settings-toggle ${checked ? "is-on" : ""}`}
    >
      <span className="settings-toggle__thumb" />
    </button>
  );
}

function GeneralSection({ settings, update }: { settings: AppSettings; update: <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => void }) {
  const { t } = useLocale();
  return (
    <div className="space-y-0">
      <h3 className="settings-section-title">{t("settings.general")}</h3>
      <SettingsRow label={t("settings.appearance")} description={t("settings.appearance.desc")}>
        <SelectControl
          value={settings.theme}
          options={[
            { value: "system", label: t("settings.theme.system") },
            { value: "light", label: t("settings.theme.light") },
            { value: "dark", label: t("settings.theme.dark") },
          ]}
          onChange={(v) => update("theme", v as AppSettings["theme"])}
        />
      </SettingsRow>
      <SettingsRow label={t("settings.accentColor")} description={t("settings.accentColor.desc")}>
        <SelectControl
          value={settings.accentColor}
          options={[
            { value: "blue", label: t("settings.color.blue") },
            { value: "purple", label: t("settings.color.purple") },
            { value: "green", label: t("settings.color.green") },
            { value: "orange", label: t("settings.color.orange") },
          ]}
          onChange={(v) => update("accentColor", v)}
        />
      </SettingsRow>
      <SettingsRow label={t("settings.language")} description={t("settings.language.desc")}>
        <SelectControl
          value={settings.language}
          options={[
            { value: "auto", label: t("settings.language.auto") },
            { value: "zh-CN", label: "简体中文" },
            { value: "en", label: "English" },
          ]}
          onChange={(v) => update("language", v as AppSettings["language"])}
        />
      </SettingsRow>
    </div>
  );
}

function NetworkSection({ settings, update }: { settings: AppSettings; update: <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => void }) {
  const { t } = useLocale();
  return (
    <div className="space-y-0">
      <h3 className="settings-section-title">{t("settings.network")}</h3>
      <SettingsRow label={t("settings.proxy")} description={t("settings.proxy.desc")}>
        <SelectControl
          value={settings.proxyMode}
          options={[
            { value: "none", label: t("settings.proxy.none") },
            { value: "system", label: t("settings.proxy.system") },
            { value: "custom", label: t("settings.proxy.custom") },
          ]}
          onChange={(v) => update("proxyMode", v as AppSettings["proxyMode"])}
        />
      </SettingsRow>
      {settings.proxyMode === "custom" ? (
        <SettingsRow label={t("settings.proxyUrl")} description={t("settings.proxyUrl.desc")}>
          <Input
            value={settings.proxyUrl}
            onChange={(e) => update("proxyUrl", e.target.value)}
            placeholder="http://127.0.0.1:7890"
            variant="secondary"
            className="w-[200px]"
            aria-label="Proxy URL"
          />
        </SettingsRow>
      ) : null}
      <SettingsRow label={t("settings.timeout")} description={t("settings.timeout.desc")}>
        <SelectControl
          value={String(settings.requestTimeout)}
          options={[
            { value: "15", label: "15s" },
            { value: "30", label: "30s" },
            { value: "60", label: "60s" },
            { value: "120", label: "120s" },
          ]}
          onChange={(v) => update("requestTimeout", Number(v))}
        />
      </SettingsRow>
    </div>
  );
}

function AccountSection({ authLogin, authScopes, onLogout }: { authLogin: string | null | undefined; authScopes: string[]; onLogout: () => void }) {
  const { t } = useLocale();
  return (
    <div className="space-y-0">
      <h3 className="settings-section-title">{t("settings.account")}</h3>
      <SettingsRow label={t("settings.githubAccount")} description={authLogin ? `${t("settings.connected")} @${authLogin}` : t("settings.notConnected")}>
        <div className="flex items-center gap-2">
          <span className="grid size-7 place-items-center rounded-full bg-[var(--brand-soft)] text-[10px] font-semibold text-[var(--brand-fg)]">
            {(authLogin ?? "?").slice(0, 2).toUpperCase()}
          </span>
          <span className="text-[12.5px] font-medium">{authLogin ? `@${authLogin}` : "—"}</span>
        </div>
      </SettingsRow>
      {authScopes.length ? (
        <SettingsRow label={t("settings.scopes")} description={t("settings.scopes.desc")}>
          <span className="text-[12px] font-mono text-[var(--fg-muted)]">{authScopes.join(", ")}</span>
        </SettingsRow>
      ) : null}
      <SettingsRow label={t("settings.logout")} description={t("settings.logout.desc")}>
        <Button size="sm" variant="outline" onPress={onLogout}>
          <LogOut size={13} />
          {t("settings.logout.btn")}
        </Button>
      </SettingsRow>
    </div>
  );
}

function AboutSection() {
  const { t } = useLocale();
  return (
    <div className="space-y-0">
      <h3 className="settings-section-title">{t("settings.about")}</h3>
      <SettingsRow label={t("settings.version")} description="Team AI Hub Desktop">
        <span className="text-[12.5px] font-mono">0.1.0</span>
      </SettingsRow>
      <SettingsRow label={t("settings.runtime")} description="Tauri + React">
        <span className="text-[12.5px] font-mono">Tauri v2</span>
      </SettingsRow>
      <SettingsRow label={t("settings.dataDir")} description="~/.team-ai-hub/">
        <div className="flex items-center gap-2">
          <span className="text-[12px] font-mono text-[var(--fg-muted)]">~/.team-ai-hub/</span>
          <Button size="sm" variant="outline" onPress={() => openDataDir()}>
            <FolderOpen size={12} />
            {t("settings.dataDir.open")}
          </Button>
        </div>
      </SettingsRow>
    </div>
  );
}

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function CacheSection() {
  const { t } = useLocale();
  const [cacheData, setCacheData] = useState<CacheSizeInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [clearing, setClearing] = useState<string | null>(null);

  const loadCacheData = async () => {
    setLoading(true);
    const data = await dbCacheStats();
    setCacheData(data);
    setLoading(false);
  };

  useEffect(() => {
    loadCacheData();
  }, []);

  const totalBytes = cacheData.reduce((sum, ws) => sum + ws.totalBytes, 0);
  const totalCount = cacheData.reduce((sum, ws) => sum + ws.count, 0);

  const handleClearWorkspace = async (workspace: string) => {
    setClearing(workspace);
    await dbClearCache(workspace);
    await loadCacheData();
    setClearing(null);
  };

  const handleClearAll = async () => {
    setClearing("__all__");
    await dbClearCache();
    await loadCacheData();
    setClearing(null);
  };

  return (
    <div className="space-y-0">
      <h3 className="settings-section-title">{t("settings.cache")}</h3>

      <SettingsRow label={t("settings.cache.totalSize")} description={t("settings.cache.totalSize.desc")}>
        <span className="text-[12.5px] font-mono">
          {loading ? t("settings.cache.calculating") : `${formatBytes(totalBytes)} · ${totalCount} ${t("settings.cache.records")}`}
        </span>
      </SettingsRow>

      {cacheData.length ? (
        <div className="border-t border-[var(--line)] pt-3 mt-3">
          <div className="text-[11px] font-medium uppercase tracking-wide text-[var(--fg-muted)] mb-2">
            {t("settings.cache.byWorkspace")}
          </div>
          <div className="space-y-1.5">
            {cacheData.map((ws) => (
              <div key={ws.workspace} className="flex items-center justify-between gap-3 rounded-md bg-[var(--bg-soft)] px-3 py-2">
                <div className="min-w-0 flex-1">
                  <div className="truncate text-[12.5px] font-medium text-[var(--fg)]">{ws.workspace}</div>
                  <div className="text-[11px] text-[var(--fg-muted)]">
                    {ws.count} {t("settings.cache.recordCount")} · {formatBytes(ws.totalBytes)}
                  </div>
                </div>
                <button
                  type="button"
                  disabled={clearing === ws.workspace}
                  onClick={() => handleClearWorkspace(ws.workspace)}
                  className="shrink-0 rounded-md p-1.5 text-[var(--fg-muted)] hover:bg-[var(--bg-active)] hover:text-[var(--danger)] disabled:opacity-50"
                  title={t("settings.cache.clearWorkspace")}
                >
                  <Trash2 size={13} />
                </button>
              </div>
            ))}
          </div>
        </div>
      ) : !loading ? (
        <div className="pt-3 text-[12px] text-[var(--fg-muted)]">{t("settings.cache.noData")}</div>
      ) : null}

      <div className="border-t border-[var(--line)] pt-3 mt-3">
        <div className="flex items-center gap-2">
          <Button size="sm" variant="outline" onPress={handleClearAll} isPending={clearing === "__all__"}>
            <Trash2 size={12} />
            {t("settings.cache.clearAll")}
          </Button>
          <Button size="sm" variant="outline" onPress={loadCacheData}>
            {t("settings.cache.refresh")}
          </Button>
        </div>
      </div>
    </div>
  );
}
