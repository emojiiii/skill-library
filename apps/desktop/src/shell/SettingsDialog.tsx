import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Button, Input, Label, ListBox, Modal, Select, Switch, toast } from "@heroui/react";
import {
  Database,
  FolderOpen,
  GitBranch,
  Github,
  Gitlab,
  Globe2,
  LogIn,
  LogOut,
  Pencil,
  Plus,
  Save,
  Server,
  Settings,
  Shield,
  Sparkles,
  Trash2,
  User,
  Wifi,
} from "lucide-react";
import { type ReactNode, useEffect, useState } from "react";
import { useLocale } from "../hooks/useLocale";
import { notifySettingsChanged } from "../hooks/useTheme";
import {
  deleteProviderInstance,
  dbCacheStats,
  dbClearCache,
  deleteAiKey,
  getAuthStatus,
  hasAiKey,
  listProviderInstances,
  loginProviderToken,
  openDataDir,
  saveAiKey,
  type CacheSizeInfo,
  type ProviderAuthStatus,
  type ProviderInstance,
  upsertProviderInstance,
  logoutProvider,
} from "../lib/skill-library";

type SettingsSection = "general" | "network" | "ai" | "cache" | "account" | "about";
type ProviderFormKind = "git-hub" | "git-lab" | "gitee" | "web-dav";

const sectionDefs: Array<{ id: SettingsSection; labelKey: string; icon: ReactNode }> = [
  { id: "account", labelKey: "settings.account", icon: <User size={15} /> },
  { id: "general", labelKey: "settings.general", icon: <Settings size={15} /> },
  { id: "network", labelKey: "settings.network", icon: <Wifi size={15} /> },
  { id: "ai", labelKey: "settings.ai", icon: <Sparkles size={15} /> },
  { id: "cache", labelKey: "settings.cache", icon: <Database size={15} /> },
  { id: "about", labelKey: "settings.about", icon: <Shield size={15} /> },
];

export interface AiProviderConfig {
  baseUrl: string;
  model: string;
}

export interface AppSettings {
  theme: "system" | "light" | "dark";
  accentColor: string;
  language: "auto" | "zh-CN" | "en";
  proxyMode: "none" | "system" | "custom";
  proxyUrl: string;
  requestTimeout: number;
  aiProvider: "none" | "openai" | "anthropic";
  aiConfigs: Record<string, AiProviderConfig>;
}

const AI_PROVIDER_DEFAULTS: Record<string, AiProviderConfig> = {
  openai: { baseUrl: "https://api.openai.com/v1", model: "gpt-5.5" },
  anthropic: { baseUrl: "https://api.anthropic.com/v1", model: "claude-opus-4-6" },
};

const defaultSettings: AppSettings = {
  theme: "system",
  accentColor: "blue",
  language: "auto",
  proxyMode: "none",
  proxyUrl: "",
  requestTimeout: 30,
  aiProvider: "none",
  aiConfigs: {
    openai: { ...AI_PROVIDER_DEFAULTS.openai },
    anthropic: { ...AI_PROVIDER_DEFAULTS.anthropic },
  },
};

/** Get the active provider's resolved config. */
export function getActiveAiConfig(settings: AppSettings): { provider: string; baseUrl: string; model: string } {
  if (settings.aiProvider === "none") return { provider: "none", baseUrl: "", model: "" };
  const config = settings.aiConfigs[settings.aiProvider];
  const defaults = AI_PROVIDER_DEFAULTS[settings.aiProvider];
  return {
    provider: settings.aiProvider,
    baseUrl: config?.baseUrl || defaults?.baseUrl || "",
    model: config?.model || defaults?.model || "",
  };
}

function loadSettings(): AppSettings {
  try {
    const raw = localStorage.getItem("skill-library-settings");
    if (raw) {
      const parsed = JSON.parse(raw);
      // Migrate from old flat format (aiBaseUrl/aiModel) to nested aiConfigs
      if (parsed.aiBaseUrl !== undefined || parsed.aiModel !== undefined) {
        const provider = parsed.aiProvider ?? "none";
        if (!parsed.aiConfigs) {
          parsed.aiConfigs = {
            openai: { ...AI_PROVIDER_DEFAULTS.openai },
            anthropic: { ...AI_PROVIDER_DEFAULTS.anthropic },
          };
        }
        if (provider !== "none" && parsed.aiConfigs[provider]) {
          if (parsed.aiBaseUrl) parsed.aiConfigs[provider].baseUrl = parsed.aiBaseUrl;
          if (parsed.aiModel) parsed.aiConfigs[provider].model = parsed.aiModel;
        }
        delete parsed.aiBaseUrl;
        delete parsed.aiModel;
      }
      return { ...defaultSettings, ...parsed };
    }
  } catch { /* ignore */ }
  return defaultSettings;
}

function saveSettings(settings: AppSettings) {
  localStorage.setItem("skill-library-settings", JSON.stringify(settings));
  notifySettingsChanged();
}

export function SettingsDialog({
  open,
  onOpenChange,
  authLogin,
  authScopes,
  onLogin,
  logoutPending,
  onLogout,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  authLogin: string | null | undefined;
  authScopes: string[];
  onLogin: () => void;
  logoutPending?: boolean;
  onLogout: () => void;
}) {
  const { t } = useLocale();
  const [section, setSection] = useState<SettingsSection>("account");
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
          <Modal.Dialog className="mx-auto rounded-[12px] bg-[var(--bg-elevated)] outline-none" style={{ width: 820, maxWidth: "92vw" }}>
            <Modal.CloseTrigger />
            <Modal.Header className="border-b border-[var(--line)] px-5 py-4">
              <Modal.Heading className="text-[15px] font-semibold">{t("settings.title")}</Modal.Heading>
            </Modal.Header>
            <Modal.Body className="p-0">
              <div className="grid h-[600px] grid-cols-[156px_1fr] divide-x divide-[var(--line)]">
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

                {/* Right content — fixed height, scrollable */}
                <div className="overflow-y-auto p-5">
                  {section === "general" ? (
                    <GeneralSection settings={settings} update={update} />
                  ) : section === "network" ? (
                    <NetworkSection settings={settings} update={update} />
                  ) : section === "ai" ? (
                    <AiSection settings={settings} update={update} />
                  ) : section === "cache" ? (
                    <CacheSection />
                  ) : section === "account" ? (
                    <AccountSection authLogin={authLogin} authScopes={authScopes} onLogin={onLogin} logoutPending={logoutPending} onLogout={onLogout} />
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

function SettingsField({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="settings-field">
      <span className="settings-field__label">{label}</span>
      {children}
    </div>
  );
}

function SelectControl({
  value,
  options,
  onChange,
  label,
  className,
  isDisabled,
}: {
  value: string;
  options: Array<{ value: string; label: string }>;
  onChange: (v: string) => void;
  label?: string;
  className?: string;
  isDisabled?: boolean;
}) {
  return (
    <Select
      value={value}
      onChange={(next) => {
        if (typeof next === "string" || typeof next === "number") {
          onChange(String(next));
        }
      }}
      variant="secondary"
      fullWidth
      className={className}
      aria-label={label ?? "Select"}
      isDisabled={isDisabled}
    >
      {label ? <Label>{label}</Label> : null}
      <Select.Trigger>
        <Select.Value />
        <Select.Indicator />
      </Select.Trigger>
      <Select.Popover>
        <ListBox>
          {options.map((opt) => (
            <ListBox.Item key={opt.value} id={opt.value} textValue={opt.label}>
              {opt.label}
              <ListBox.ItemIndicator />
            </ListBox.Item>
          ))}
        </ListBox>
      </Select.Popover>
    </Select>
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
            aria-label={t("settings.proxyUrl.aria")}
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

function AiSection({ settings, update }: { settings: AppSettings; update: <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => void }) {
  const { t } = useLocale();
  const [keyInput, setKeyInput] = useState("");
  const [keyStored, setKeyStored] = useState(false);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    hasAiKey().then(setKeyStored).catch(() => setKeyStored(false));
  }, []);

  const provider = settings.aiProvider;
  const enabled = provider !== "none";
  const currentConfig = enabled ? settings.aiConfigs[provider] : null;

  const updateProviderConfig = (field: "baseUrl" | "model", value: string) => {
    if (!enabled) return;
    const next = {
      ...settings.aiConfigs,
      [provider]: { ...settings.aiConfigs[provider], [field]: value },
    };
    update("aiConfigs", next);
  };

  const onProviderChange = (newProvider: string) => {
    update("aiProvider", newProvider as AppSettings["aiProvider"]);
  };

  const onSaveKey = async () => {
    if (!keyInput.trim()) return;
    setBusy(true);
    try {
      await saveAiKey(keyInput.trim());
      setKeyStored(true);
      setKeyInput("");
    } finally {
      setBusy(false);
    }
  };

  const onClearKey = async () => {
    setBusy(true);
    try {
      await deleteAiKey();
      setKeyStored(false);
      setKeyInput("");
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="space-y-0">
      <h3 className="settings-section-title">{t("settings.ai")}</h3>
      <p className="mb-3 text-[12px] text-[var(--fg-muted)]">{t("settings.ai.desc")}</p>

      <SettingsRow label={t("settings.ai.provider")} description={t("settings.ai.provider.desc")}>
        <SelectControl
          value={provider}
          options={[
            { value: "none", label: t("settings.ai.provider.none") },
            { value: "openai", label: "OpenAI" },
            { value: "anthropic", label: "Anthropic" },
          ]}
          onChange={onProviderChange}
        />
      </SettingsRow>

      {enabled && currentConfig ? (
        <>
          <div className="settings-row flex-col !items-start gap-1.5">
            <div>
              <div className="text-[13px] font-medium text-[var(--fg)]">{t("settings.ai.baseUrl")}</div>
              <div className="mt-0.5 text-[11.5px] text-[var(--fg-muted)]">{t("settings.ai.baseUrl.desc")}</div>
            </div>
            <Input
              value={currentConfig.baseUrl}
              onChange={(e) => updateProviderConfig("baseUrl", e.target.value)}
              placeholder={AI_PROVIDER_DEFAULTS[provider]?.baseUrl ?? "https://…"}
              variant="secondary"
              className="w-full"
              aria-label={t("settings.ai.baseUrl.aria")}
              autoCapitalize="none"
              autoCorrect="off"
              spellCheck={false}
            />
          </div>

          <div className="settings-row flex-col !items-start gap-1.5">
            <div>
              <div className="text-[13px] font-medium text-[var(--fg)]">{t("settings.ai.model")}</div>
              <div className="mt-0.5 text-[11.5px] text-[var(--fg-muted)]">{t("settings.ai.model.desc")}</div>
            </div>
            <Input
              value={currentConfig.model}
              onChange={(e) => updateProviderConfig("model", e.target.value)}
              placeholder={AI_PROVIDER_DEFAULTS[provider]?.model ?? "model"}
              variant="secondary"
              className="w-full"
              aria-label={t("settings.ai.model.aria")}
              autoCapitalize="none"
              autoCorrect="off"
              spellCheck={false}
            />
          </div>

          <div className="settings-row flex-col !items-start gap-1.5">
            <div>
              <div className="text-[13px] font-medium text-[var(--fg)]">{t("settings.ai.apiKey")}</div>
              <div className="mt-0.5 text-[11.5px] text-[var(--fg-muted)]">{keyStored ? t("settings.ai.apiKey.stored") : t("settings.ai.apiKey.desc")}</div>
            </div>
            <div className="flex w-full items-center gap-2">
              <Input
                type="password"
                value={keyInput}
                onChange={(e) => setKeyInput(e.target.value)}
                placeholder={keyStored ? "••••••••" : "sk-…"}
                variant="secondary"
                className="flex-1"
                aria-label={t("settings.ai.apiKey.aria")}
                autoCapitalize="none"
                autoCorrect="off"
                spellCheck={false}
              />
              {keyStored ? (
                <Button size="sm" variant="outline" onPress={onClearKey} isPending={busy}>
                  {t("settings.ai.apiKey.clear")}
                </Button>
              ) : (
                <Button size="sm" variant="secondary" onPress={onSaveKey} isPending={busy} isDisabled={!keyInput.trim()}>
                  {t("settings.ai.apiKey.save")}
                </Button>
              )}
            </div>
          </div>
        </>
      ) : null}
    </div>
  );
}

function providerFormDefaults(kind: ProviderFormKind) {
  if (kind === "git-hub") {
    return {
      id: "github.company.com",
      displayName: "GitHub Enterprise",
      webBaseUrl: "https://github.company.com",
      apiBaseUrl: "https://github.company.com/api/v3",
      authMode: "personal_access_token",
    };
  }
  if (kind === "gitee") {
    return {
      id: "gitee.company.com",
      displayName: "Gitee Enterprise",
      webBaseUrl: "https://gitee.company.com",
      apiBaseUrl: "https://gitee.company.com/api/v5",
      authMode: "personal_access_token",
    };
  }
  if (kind === "web-dav") {
    return {
      id: "webdav-main",
      displayName: "WebDAV",
      webBaseUrl: "https://dav.example.com",
      apiBaseUrl: "https://dav.example.com/remote.php/dav/files/user",
      authMode: "basic",
    };
  }
  return {
    id: "gitlab.company.com",
    displayName: "GitLab Enterprise",
    webBaseUrl: "https://gitlab.company.com",
    apiBaseUrl: "https://gitlab.company.com/api/v4",
    authMode: "personal_access_token",
  };
}

function defaultProviderForm(kind: ProviderFormKind = "git-lab") {
  return {
    kind,
    ...providerFormDefaults(kind),
    login: "",
    secret: "",
  };
}

function providerIdPrefix(kind: ProviderFormKind) {
  if (kind === "git-hub") return "github";
  if (kind === "git-lab") return "gitlab";
  if (kind === "web-dav") return "webdav";
  return "gitee";
}

function randomProviderIdSuffix() {
  const bytes = new Uint8Array(3);
  if (globalThis.crypto?.getRandomValues) {
    globalThis.crypto.getRandomValues(bytes);
    return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
  }
  return Math.floor(Math.random() * 0xffffff).toString(16).padStart(6, "0");
}

function generateProviderInstanceId(kind: ProviderFormKind, existingIds: Iterable<string>) {
  const prefix = providerIdPrefix(kind);
  const existing = new Set(Array.from(existingIds, (id) => id.toLowerCase()));
  for (let attempt = 0; attempt < 10; attempt += 1) {
    const id = `${prefix}-${randomProviderIdSuffix()}`;
    if (!existing.has(id.toLowerCase())) return id;
  }
  return `${prefix}-${Date.now().toString(36).slice(-6)}`;
}

function providerFormKindFromInstance(instance: ProviderInstance): ProviderFormKind {
  const kind = providerKindValue(instance).toLowerCase();
  if (kind === "git-hub" || kind === "git-lab" || kind === "gitee" || kind === "web-dav") {
    return kind;
  }
  return "git-lab";
}

function providerFormFromInstance(instance: ProviderInstance) {
  return {
    kind: providerFormKindFromInstance(instance),
    id: instance.id,
    displayName: instance.displayName,
    webBaseUrl: instance.webBaseUrl,
    apiBaseUrl: instance.apiBaseUrl,
    authMode: firstAuthMode(instance),
    login: "",
    secret: "",
  };
}

function isBuiltInProvider(instance: ProviderInstance) {
  return instance.id === "github.com" || instance.id === "gitlab.com" || instance.id === "gitee.com";
}

function AccountSection({
  authLogin,
  authScopes,
  onLogin,
  logoutPending,
  onLogout,
}: {
  authLogin: string | null | undefined;
  authScopes: string[];
  onLogin: () => void;
  logoutPending?: boolean;
  onLogout: () => void;
}) {
  const { t } = useLocale();
  const queryClient = useQueryClient();
  const instancesQuery = useQuery({
    queryKey: ["provider-instances"],
    queryFn: listProviderInstances,
    enabled: true,
  });
  const authStatus = useQuery({
    queryKey: ["auth-status"],
    queryFn: getAuthStatus,
    enabled: true,
  });
  const [addProviderOpen, setAddProviderOpen] = useState(false);
  const [editingProviderId, setEditingProviderId] = useState<string | null>(null);
  const [providerForm, setProviderForm] = useState(defaultProviderForm());
  const fallbackInstances: ProviderInstance[] = [
    {
      id: "github.com",
      kind: "git-hub",
      displayName: "GitHub",
      webBaseUrl: "https://github.com",
      apiBaseUrl: "https://api.github.com",
      authModes: ["personal_access_token", "device_flow"],
      enabled: true,
    },
    {
      id: "gitlab.com",
      kind: "git-lab",
      displayName: "GitLab.com",
      webBaseUrl: "https://gitlab.com",
      apiBaseUrl: "https://gitlab.com/api/v4",
      authModes: ["personal_access_token"],
      enabled: true,
    },
    {
      id: "gitee.com",
      kind: "gitee",
      displayName: "Gitee",
      webBaseUrl: "https://gitee.com",
      apiBaseUrl: "https://gitee.com/api/v5",
      authModes: ["personal_access_token"],
      enabled: true,
    },
  ];
  const instances = instancesQuery.data?.length ? instancesQuery.data : fallbackInstances;
  const providerStatuses = authStatus.data?.providers ?? [];
  const visibleProviders = instances
    .filter((instance) => {
      if (instance.id === "github.com") return true;
      if (!isBuiltInProvider(instance)) return true;
      return Boolean(providerStatuses.find((status) => status.provider === instance.id)?.authenticated);
    })
    .sort((a, b) => providerSortRank(a.id) - providerSortRank(b.id) || a.displayName.localeCompare(b.displayName));

  const refreshProviders = async () => {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: ["auth-status"] }),
      queryClient.invalidateQueries({ queryKey: ["provider-instances"] }),
      queryClient.invalidateQueries({ queryKey: ["provider-workspaces"] }),
    ]);
  };

  const tokenLogin = useMutation({
    mutationFn: ({
      providerId,
      token,
      authMode,
      login,
    }: {
      providerId: string;
      token: string;
      authMode?: string;
      login?: string;
    }) => loginProviderToken(providerId, token, { authMode, login }),
    onSuccess: async (_status, variables) => {
      await refreshProviders();
      toast.success(t("settings.provider.saved"));
    },
    onError: (err) => toast.danger(String((err as { message?: string })?.message ?? err)),
  });

  const providerLogout = useMutation({
    mutationFn: logoutProvider,
    onSuccess: async () => {
      await refreshProviders();
      toast.success(t("settings.provider.loggedOut"));
    },
    onError: (err) => toast.danger(String((err as { message?: string })?.message ?? err)),
  });

  const saveInstance = useMutation({
    mutationFn: upsertProviderInstance,
    onSuccess: async () => {
      await refreshProviders();
    },
    onError: (err) => toast.danger(String((err as { message?: string })?.message ?? err)),
  });

  const removeInstance = useMutation({
    mutationFn: deleteProviderInstance,
    onSuccess: async () => {
      await refreshProviders();
      toast.success(t("settings.provider.instanceDeleted"));
    },
    onError: (err) => toast.danger(String((err as { message?: string })?.message ?? err)),
  });

  const statusFor = (instance: ProviderInstance): ProviderAuthStatus | null => {
    const fromQuery = providerStatuses.find((status) => status.provider === instance.id);
    if (fromQuery) return fromQuery;
    if (instance.id === "github.com" && authLogin) {
      return {
        provider: "github.com",
        displayName: "GitHub",
        login: authLogin,
        scopes: authScopes,
        authMode: "device_flow",
        authenticated: true,
      };
    }
    return null;
  };

  const providerFormUsesLoginSecret =
    providerForm.kind === "web-dav" && usesWebDavLoginSecret(providerForm.authMode);
  const providerFormSecretLabel =
    providerForm.kind !== "web-dav"
      ? t("settings.provider.tokenOptional")
      : providerForm.authMode === "app_password"
        ? t("settings.webdav.appPassword")
        : providerFormUsesLoginSecret
          ? t("settings.webdav.password")
          : t("settings.webdav.bearerToken");
  const providerFormDefaultsValue = providerFormDefaults(providerForm.kind);
  const canSaveProvider = Boolean(
    providerForm.id.trim() &&
    providerForm.displayName.trim() &&
    providerForm.webBaseUrl.trim() &&
    providerForm.apiBaseUrl.trim() &&
    (!providerForm.secret.trim() || !providerFormUsesLoginSecret || providerForm.login.trim()),
  );
  const providerKindOptions = [
    { value: "git-hub", label: t("settings.provider.kind.github") },
    { value: "git-lab", label: t("settings.provider.kind.gitlab") },
    { value: "gitee", label: t("settings.provider.kind.gitee") },
    { value: "web-dav", label: t("settings.provider.kind.webdav") },
  ];
  const providerAuthModeOptions = [
    { value: "basic", label: t("settings.webdav.auth.basic") },
    { value: "app_password", label: t("settings.webdav.auth.appPassword") },
    { value: "personal_access_token", label: t("settings.webdav.auth.token") },
  ];
  const editingProvider = editingProviderId
    ? instances.find((instance) => instance.id === editingProviderId) ?? null
    : null;
  const isEditingProvider = Boolean(editingProviderId);

  const defaultNewProviderForm = (kind: ProviderFormKind = "git-lab") => ({
    ...defaultProviderForm(kind),
    id: generateProviderInstanceId(kind, instances.map((instance) => instance.id)),
  });

  const openAddProvider = () => {
    setEditingProviderId(null);
    setProviderForm(defaultNewProviderForm());
    setAddProviderOpen(true);
  };

  const openEditProvider = (instance: ProviderInstance) => {
    setEditingProviderId(instance.id);
    setProviderForm(providerFormFromInstance(instance));
    setAddProviderOpen(true);
  };

  const closeProviderDialog = () => {
    setAddProviderOpen(false);
    setEditingProviderId(null);
    setProviderForm(defaultNewProviderForm());
  };

  const onToggleProviderEnabled = (instance: ProviderInstance, enabled: boolean) => {
    saveInstance.mutate(
      { ...instance, enabled },
      {
        onSuccess: async () => {
          await refreshProviders();
          toast.success(enabled ? t("settings.provider.enabledSaved") : t("settings.provider.disabledSaved"));
        },
      },
    );
  };

  const onSaveProvider = () => {
    if (!canSaveProvider) return;
    const id = editingProviderId ?? providerForm.id.trim();
    const instance = {
      id,
      kind: providerForm.kind,
      displayName: providerForm.displayName.trim(),
      webBaseUrl: providerForm.webBaseUrl.trim(),
      apiBaseUrl: providerForm.apiBaseUrl.trim(),
      authModes: [providerForm.authMode],
      enabled: editingProvider?.enabled ?? true,
    };
    saveInstance.mutate(instance, {
      onSuccess: () => {
        const secret = providerForm.secret.trim();
        closeProviderDialog();
        if (!secret) {
          toast.success(t("settings.provider.instanceSaved"));
          return;
        }
        tokenLogin.mutate({
          providerId: id,
          token: secret,
          authMode: providerForm.authMode,
          login: providerFormUsesLoginSecret ? providerForm.login.trim() : undefined,
        });
      },
    });
  };

  return (
    <>
      <div className="space-y-4">
        <div className="flex items-center justify-between gap-3">
          <h3 className="settings-section-title !mb-0">{t("settings.account")}</h3>
          <Button
            size="sm"
            variant="secondary"
            onPress={openAddProvider}
          >
            <Plus size={13} />
            {t("settings.provider.add")}
          </Button>
        </div>

        <div className="space-y-2">
          {visibleProviders.map((instance) => {
            const status = statusFor(instance);
            const connected = Boolean(status?.authenticated);
            const isGithubOauth = instance.id === "github.com";
            const isCustomProvider = !isBuiltInProvider(instance);
            const scopes = status?.scopes ?? [];
            const statusLabel = !instance.enabled
              ? t("settings.provider.disabled")
              : connected
                ? t("settings.connected")
                : t("settings.notConnected");
            const statusClass = !instance.enabled ? "is-disabled" : connected ? "is-connected" : "";
            return (
              <div className="settings-provider-card" key={instance.id}>
                <div className="flex min-w-0 flex-1 items-start gap-3">
                  <span className="settings-provider-icon">{providerIcon(instance)}</span>
                  <div className="min-w-0 flex-1">
                    <div className="flex min-w-0 flex-wrap items-center gap-2">
                      <div className="truncate text-[13px] font-semibold text-[var(--fg)]">{instance.displayName}</div>
                      <span className={`settings-provider-status ${statusClass}`}>
                        {statusLabel}
                      </span>
                    </div>
                    <div className="mt-0.5 truncate text-[11.5px] text-[var(--fg-muted)]">
                      {connected && status?.login ? `@${status.login}` : instance.webBaseUrl}
                    </div>
                    {connected && scopes.length ? (
                      <div className="mt-2 truncate font-mono text-[11px] text-[var(--fg-muted)]">
                        {scopes.join(", ")}
                      </div>
                    ) : null}
                  </div>
                </div>
                <div className="flex shrink-0 items-center gap-2">
                  {isCustomProvider ? (
                    <>
                      <label className="settings-provider-switch">
                        <Switch
                          isSelected={instance.enabled}
                          isDisabled={saveInstance.isPending}
                          onChange={(enabled) => onToggleProviderEnabled(instance, enabled)}
                        >
                          <Switch.Control>
                            <Switch.Thumb />
                          </Switch.Control>
                        </Switch>
                        <span>{instance.enabled ? t("settings.provider.enabledShort") : t("settings.provider.disabledShort")}</span>
                      </label>
                      <Button
                        size="sm"
                        variant="outline"
                        onPress={() => openEditProvider(instance)}
                      >
                        <Pencil size={13} />
                        {t("settings.provider.edit")}
                      </Button>
                      <Button
                        size="sm"
                        variant="outline"
                        onPress={() => removeInstance.mutate(instance.id)}
                        isPending={removeInstance.isPending}
                      >
                        <Trash2 size={13} />
                        {t("settings.provider.delete")}
                      </Button>
                    </>
                  ) : connected ? (
                    <Button
                      size="sm"
                      variant="outline"
                      onPress={isGithubOauth ? onLogout : () => providerLogout.mutate(instance.id)}
                      isPending={(isGithubOauth && logoutPending) || providerLogout.isPending}
                    >
                      <LogOut size={13} />
                      {t("settings.logout.btn")}
                    </Button>
                  ) : isGithubOauth ? (
                    <Button size="sm" variant="secondary" onPress={onLogin}>
                      <LogIn size={13} />
                      {t("auth.continueWithGithub")}
                    </Button>
                  ) : null}
                </div>
              </div>
            );
          })}
        </div>
      </div>

      <Modal isOpen={addProviderOpen} onOpenChange={(open) => (open ? setAddProviderOpen(true) : closeProviderDialog())}>
        <Modal.Backdrop>
          <Modal.Container size="md">
            <Modal.Dialog className="rounded-[12px] bg-[var(--bg-elevated)] outline-none">
              <Modal.CloseTrigger />
              <Modal.Header className="border-b border-[var(--line)] px-5 py-4">
                <Modal.Heading className="text-[15px] font-semibold tracking-tight">
                  {isEditingProvider ? t("settings.provider.editTitle") : t("settings.provider.addTitle")}
                </Modal.Heading>
                <div className="mt-1 text-[12px] text-[var(--fg-muted)]">
                  {isEditingProvider ? t("settings.provider.editDesc") : t("settings.provider.addDesc")}
                </div>
              </Modal.Header>
              <Modal.Body className="space-y-3 px-5 py-4">
                <SelectControl
                  value={providerForm.kind}
                  label={t("settings.provider.kind")}
                  options={providerKindOptions}
                  onChange={(value) => setProviderForm(defaultNewProviderForm(value as ProviderFormKind))}
                  isDisabled={isEditingProvider}
                />
                <SettingsField label={t("settings.provider.name")}>
                  <Input
                    value={providerForm.displayName}
                    onChange={(e) => setProviderForm((next) => ({ ...next, displayName: e.target.value }))}
                    placeholder={providerFormDefaultsValue.displayName}
                    variant="secondary"
                    aria-label={t("settings.provider.name")}
                    fullWidth
                  />
                </SettingsField>
                <SettingsField label={t("settings.provider.webUrl")}>
                  <Input
                    value={providerForm.webBaseUrl}
                    onChange={(e) => setProviderForm((next) => ({ ...next, webBaseUrl: e.target.value }))}
                    placeholder={providerFormDefaultsValue.webBaseUrl}
                    variant="secondary"
                    aria-label={t("settings.provider.webUrl")}
                    autoCapitalize="none"
                    autoCorrect="off"
                    spellCheck={false}
                    fullWidth
                  />
                </SettingsField>
                <SettingsField label={providerForm.kind === "web-dav" ? t("settings.provider.webdavUrl") : t("settings.provider.apiUrl")}>
                  <Input
                    value={providerForm.apiBaseUrl}
                    onChange={(e) => setProviderForm((next) => ({ ...next, apiBaseUrl: e.target.value }))}
                    placeholder={providerFormDefaultsValue.apiBaseUrl}
                    variant="secondary"
                    aria-label={providerForm.kind === "web-dav" ? t("settings.provider.webdavUrl") : t("settings.provider.apiUrl")}
                    autoCapitalize="none"
                    autoCorrect="off"
                    spellCheck={false}
                    fullWidth
                  />
                </SettingsField>
                {providerForm.kind === "web-dav" ? (
                  <SelectControl
                    value={providerForm.authMode}
                    label={t("settings.provider.authMode")}
                    options={providerAuthModeOptions}
                    onChange={(value) =>
                      setProviderForm((next) => ({
                        ...next,
                        authMode: value,
                        login: "",
                        secret: "",
                      }))
                    }
                  />
                ) : null}
                {providerFormUsesLoginSecret ? (
                  <SettingsField label={t("settings.webdav.username")}>
                    <Input
                      value={providerForm.login}
                      onChange={(e) => setProviderForm((next) => ({ ...next, login: e.target.value }))}
                      placeholder={t("settings.webdav.username")}
                      variant="secondary"
                      aria-label={t("settings.webdav.username")}
                      autoCapitalize="none"
                      autoCorrect="off"
                      spellCheck={false}
                      fullWidth
                    />
                  </SettingsField>
                ) : null}
                <SettingsField label={providerFormSecretLabel}>
                  <Input
                    type="password"
                    value={providerForm.secret}
                    onChange={(e) => setProviderForm((next) => ({ ...next, secret: e.target.value }))}
                    placeholder={providerFormSecretLabel}
                    variant="secondary"
                    aria-label={providerFormSecretLabel}
                    autoCapitalize="none"
                    autoCorrect="off"
                    spellCheck={false}
                    fullWidth
                  />
                </SettingsField>
              </Modal.Body>
              <div className="flex justify-end gap-2 border-t border-[var(--line)] px-5 py-3">
                <Button variant="outline" onPress={closeProviderDialog}>
                  {t("common.cancel")}
                </Button>
                <Button
                  onPress={onSaveProvider}
                  isPending={saveInstance.isPending || tokenLogin.isPending}
                  isDisabled={!canSaveProvider}
                >
                  <Save size={13} />
                  {isEditingProvider ? t("settings.provider.saveChanges") : t("settings.provider.save")}
                </Button>
              </div>
            </Modal.Dialog>
          </Modal.Container>
        </Modal.Backdrop>
      </Modal>
    </>
  );
}

function providerKindValue(instance: ProviderInstance) {
  return typeof instance.kind === "string" ? instance.kind : instance.kind.custom;
}

function isWebDavProvider(instance: ProviderInstance) {
  return providerKindValue(instance).toLowerCase() === "web-dav";
}

function providerSortRank(id: string) {
  if (id === "github.com") return 0;
  if (id === "gitlab.com") return 1;
  if (id === "gitee.com") return 2;
  return 10;
}

function firstAuthMode(instance: ProviderInstance) {
  return instance.authModes[0] ?? "personal_access_token";
}

function usesWebDavLoginSecret(authMode: string) {
  return authMode === "basic" || authMode === "app_password";
}

function providerIcon(instance: ProviderInstance) {
  const kind = providerKindValue(instance).toLowerCase();
  if (kind === "git-hub") return <Github size={15} />;
  if (kind === "git-lab") return <Gitlab size={15} />;
  if (kind === "gitee") return <GitBranch size={15} />;
  if (kind === "web-dav") return <Server size={15} />;
  return <Globe2 size={15} />;
}

function AboutSection() {
  const { t } = useLocale();
  return (
    <div className="space-y-0">
      <h3 className="settings-section-title">{t("settings.about")}</h3>
      <SettingsRow label={t("settings.version")} description="Skill Library Desktop">
        <span className="text-[12.5px] font-mono">0.1.0</span>
      </SettingsRow>
      <SettingsRow label={t("settings.runtime")} description="Tauri + React">
        <span className="text-[12.5px] font-mono">Tauri v2</span>
      </SettingsRow>
      <SettingsRow label={t("settings.dataDir")} description="~/.skill-library/">
        <div className="flex items-center gap-2">
          <span className="text-[12px] font-mono text-[var(--fg-muted)]">~/.skill-library/</span>
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
        <div className="pt-3">
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
          <Button
            size="sm"
            variant="outline"
            onPress={() => {
              const raw = localStorage.getItem("skill-library-settings");
              if (raw) {
                try {
                  const parsed = JSON.parse(raw);
                  parsed.aiConfigs = {
                    openai: { ...AI_PROVIDER_DEFAULTS.openai },
                    anthropic: { ...AI_PROVIDER_DEFAULTS.anthropic },
                  };
                  localStorage.setItem("skill-library-settings", JSON.stringify(parsed));
                  notifySettingsChanged();
                  toast.success(t("settings.cache.aiConfig.success"));
                } catch {
                  toast.danger(t("settings.cache.aiConfig.fail"));
                }
              }
            }}
          >
            <Trash2 size={12} />
            {t("settings.cache.aiConfig.reset")}
          </Button>
        </div>
      </div>
    </div>
  );
}
