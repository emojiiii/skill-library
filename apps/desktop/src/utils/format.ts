import { openUrl } from "@tauri-apps/plugin-opener";

type StructuredErrorLike = {
  code?: unknown;
  message?: unknown;
  error?: unknown;
  error_description?: unknown;
  reason?: unknown;
  hint?: unknown;
  scope?: unknown;
};

export const formatError = (error: unknown): string => {
  if (error instanceof Error) {
    return error.message;
  }
  if (error && typeof error === "object") {
    const value = error as StructuredErrorLike;
    if (value.error) {
      const formattedError = formatError(value.error);
      const description = typeof value.error_description === "string" ? value.error_description : null;
      const scope = typeof value.scope === "string" ? value.scope : null;
      return [formattedError, description, scope ? `required scope: ${scope}` : null]
        .filter(Boolean)
        .join(" - ");
    }
    const code = typeof value.code === "string" ? value.code : null;
    const message = value.message === undefined ? null : formatError(value.message);
    const reason = value.reason === undefined ? null : formatError(value.reason);
    const hint = value.hint === undefined ? null : formatError(value.hint);
    if (code && message) {
      return `${code}: ${message}`;
    }
    if (message) {
      return message;
    }
    if (code) {
      return code;
    }
    if (reason) {
      return reason;
    }
    if (hint) {
      return hint;
    }
    try {
      const serialized = JSON.stringify(error);
      if (serialized && serialized !== "{}") return serialized;
    } catch {
      // Fall through to the String() fallback below.
    }
  }
  return String(error);
};

export const openExternalUrl = async (url: string) => {
  try {
    await openUrl(url);
  } catch {
    window.open(url, "_blank", "noopener,noreferrer");
  }
};

export function shortHash(value?: string | null): string {
  if (!value) return "-";
  return value.startsWith("sha256:") ? `${value.slice(0, 19)}...` : value.slice(0, 16);
}

/** Compact install count: 1234 -> "1.2k", 1759035 -> "1.8m". */
export function formatInstalls(value: number): string {
  if (!Number.isFinite(value) || value <= 0) return "0";
  if (value < 1000) return String(value);
  if (value < 1_000_000) return `${(value / 1000).toFixed(value < 10_000 ? 1 : 0)}k`;
  return `${(value / 1_000_000).toFixed(1)}m`;
}

export function formatDateTime(value?: string | null): string {
  if (!value) return "-";
  return new Intl.DateTimeFormat(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(value));
}

function intlLocale(locale?: string): string | undefined {
  if (locale === "zh") return "zh-CN";
  if (locale === "en") return "en";
  return undefined;
}

export function formatRelativeTime(value?: string | null, locale?: string): string {
  if (!value) return "-";
  const target = new Date(value).getTime();
  if (Number.isNaN(target)) return "-";
  const diff = Date.now() - target;
  const rtf = new Intl.RelativeTimeFormat(intlLocale(locale), {
    numeric: "auto",
    style: "narrow",
  });
  const minutes = Math.max(0, Math.round(diff / 60000));
  if (minutes < 60) return rtf.format(-minutes, "minute");
  const hours = Math.round(minutes / 60);
  if (hours < 24) return rtf.format(-hours, "hour");
  const days = Math.round(hours / 24);
  if (days < 7) return rtf.format(-days, "day");
  return formatDateTime(value);
}
