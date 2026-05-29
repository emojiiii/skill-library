import { openUrl } from "@tauri-apps/plugin-opener";

type StructuredErrorLike = {
  code?: unknown;
  message?: unknown;
  error?: unknown;
};

export const formatError = (error: unknown): string => {
  if (error instanceof Error) {
    return error.message;
  }
  if (error && typeof error === "object") {
    const value = error as StructuredErrorLike;
    if (value.error) {
      return formatError(value.error);
    }
    const code = typeof value.code === "string" ? value.code : null;
    const message = typeof value.message === "string" ? value.message : null;
    if (code && message) {
      return `${code}: ${message}`;
    }
    if (message) {
      return message;
    }
    if (code) {
      return code;
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
  if (!value) return "no hash";
  return value.startsWith("sha256:") ? `${value.slice(0, 19)}...` : value.slice(0, 16);
}

export function formatDateTime(value?: string | null): string {
  if (!value) return "unknown";
  return new Intl.DateTimeFormat(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(value));
}

export function formatRelativeTime(value?: string | null): string {
  if (!value) return "unknown";
  const target = new Date(value).getTime();
  if (Number.isNaN(target)) return "unknown";
  const diff = Date.now() - target;
  const minutes = Math.round(diff / 60000);
  if (minutes < 1) return "just now";
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.round(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.round(hours / 24);
  if (days < 7) return `${days}d ago`;
  return formatDateTime(value);
}
