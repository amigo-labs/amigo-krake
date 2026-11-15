// Svelte stores for application state
import { writable, derived, get } from "svelte/store";

// ========================================
// TYPES (single source of truth — audit M6)
// ========================================

export interface Download {
  id: string;
  url: string;
  protocol: string;
  filename: string | null;
  filesize: number | null;
  status: string;
  priority: number;
  bytes_downloaded: number;
  speed: number;
  error: string | null;
  created_at: string;
}

export interface Stats {
  active_downloads: number;
  speed_bytes_per_sec: number;
  queued: number;
  completed: number;
}

export interface CaptchaChallenge {
  id: string;
  plugin_id: string;
  download_id: string;
  image_url: string;
  captcha_type: string;
}

export interface UsenetServer {
  id: string;
  name: string;
  host: string;
  port: number;
  ssl: boolean;
  connections: number;
  priority: number;
}

export interface Features {
  usenet: boolean;
  rss_feeds: boolean;
  server_stats: boolean;
}

// ========================================
// NAVIGATION
// ========================================

export type Page = "downloads" | "plugins" | "history" | "settings";
export type ProtocolFilter = "all" | "http" | "usenet";

const validPages: Page[] = ["downloads", "plugins", "history", "settings"];

function pageFromHash(): Page {
  const hash = typeof location !== "undefined" ? location.hash.slice(1) : "";
  return validPages.includes(hash as Page) ? (hash as Page) : "downloads";
}

export const currentPage = writable<Page>(pageFromHash());
export const protocolFilter = writable<ProtocolFilter>("all");

// Sync URL hash on every page change (subscriptions have no teardown
// concern — Svelte stores keep the subscription as long as the store lives).
if (typeof window !== "undefined") {
  currentPage.subscribe((page) => {
    if (location.hash !== `#${page}`) {
      history.pushState({ page }, "", `#${page}`);
    }
    document.title = `${page.charAt(0).toUpperCase() + page.slice(1)} — Krake`;
  });
}

/**
 * Wire the browser's `popstate` event to the {@link currentPage} store.
 *
 * Returns an unsubscriber that removes the listener. Call from a Svelte
 * component's `onMount` and surface the returned function as the cleanup,
 * so HMR / component teardown / tests don't leak listeners on `window`.
 *
 * Previous code attached the listener at module-init time with no removal
 * path. In production that's a singleton (one tab, one window), but during
 * HMR each module reload would stack another listener on `window`.
 */
export function attachRouterPopstateListener(): () => void {
  if (typeof window === "undefined") {
    return () => {};
  }
  const handler = () => {
    currentPage.set(pageFromHash());
  };
  window.addEventListener("popstate", handler);
  return () => {
    window.removeEventListener("popstate", handler);
  };
}

// ========================================
// DOWNLOADS DATA
// ========================================

export const downloads = writable<Download[]>([]);
export const selectedDownloadId = writable<string | null>(null);

/**
 * Flips true after the first downloads fetch *attempt* completes (success or
 * failure). Drives skeleton gating: once an attempt has finished, the Downloads
 * page shows real data or its empty/offline state rather than skeletons forever.
 */
export const downloadsLoaded = writable<boolean>(false);

export const selectedDownload = derived(
  [downloads, selectedDownloadId],
  ([$downloads, $id]) => $id ? $downloads.find((d) => d.id === $id) ?? null : null
);

/** Update a single download's progress in-place (from WebSocket events). */
export function updateDownloadProgress(id: string, bytes_downloaded: number, speed: number, total_bytes?: number) {
  downloads.update((list) =>
    list.map((d) =>
      d.id === id
        ? { ...d, bytes_downloaded, speed, filesize: total_bytes ?? d.filesize }
        : d
    )
  );
}

/** Mark a download's status (from WebSocket events). */
export function updateDownloadStatus(id: string, status: string) {
  downloads.update((list) =>
    list.map((d) => (d.id === id ? { ...d, status } : d))
  );
}

// ========================================
// STATS
// ========================================

export const stats = writable<Stats>({
  active_downloads: 0,
  speed_bytes_per_sec: 0,
  queued: 0,
  completed: 0,
});

// Tab badge — show active count in document title
if (typeof window !== "undefined") {
  stats.subscribe((s) => {
    const hash = typeof location !== "undefined" ? location.hash.slice(1) : "downloads";
    const page = hash || "downloads";
    const prefix = s.active_downloads > 0 ? `(${s.active_downloads}) ` : "";
    document.title = `${prefix}${page.charAt(0).toUpperCase() + page.slice(1)} — Krake`;
  });
}

// ========================================
// SPEED HISTORY (for sparkline graph)
// ========================================

const MAX_SPEED_HISTORY = 30;
export const speedHistory = writable<number[]>([]);

export function pushSpeedSample(speed: number) {
  speedHistory.update((h) => {
    const next = [...h, speed];
    return next.length > MAX_SPEED_HISTORY ? next.slice(-MAX_SPEED_HISTORY) : next;
  });
}

// ========================================
// BATCH SELECTION
// ========================================

export const selectedIds = writable<Set<string>>(new Set());

export function toggleSelection(id: string) {
  selectedIds.update((s) => {
    const next = new Set(s);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    return next;
  });
}

export function clearSelection() {
  selectedIds.set(new Set());
}

export function selectAll(ids: string[]) {
  selectedIds.set(new Set(ids));
}

// ========================================
// SEARCH
// ========================================

export const searchQuery = writable<string>("");

// ========================================
// CAPTCHA
// ========================================

export const pendingCaptcha = writable<CaptchaChallenge | null>(null);

// ========================================
// USENET
// ========================================

export const usenetServers = writable<UsenetServer[]>([]);
export const usenetDownloads = writable<Download[]>([]);

// ========================================
// FEATURE FLAGS
// ========================================

export const features = writable<Features>({
  usenet: false,
  rss_feeds: false,
  server_stats: false,
});

// ========================================
// SIDE PANEL
// ========================================

export type SidePanelMode = "detail" | "add" | null;

export const sidePanelMode = writable<SidePanelMode>(null);

export function openAddPanel() {
  selectedDownloadId.set(null);
  sidePanelMode.set("add");
}

export function openDetailPanel(id: string) {
  selectedDownloadId.set(id);
  sidePanelMode.set("detail");
}

export function closeSidePanel() {
  sidePanelMode.set(null);
  selectedDownloadId.set(null);
}

// ========================================
// CRASH REPORT (audit M5 — replaces window.__amigo_report_crash)
// ========================================

export interface CrashContext {
  download_id?: string;
  error_message?: string;
  url?: string;
}

export const crashReport = writable<CrashContext | null>(null);
export const showFeedbackDialog = writable<boolean>(false);

// ========================================
// WEBSOCKET CONNECTION STATUS
// ========================================

export const wsConnected = writable<boolean>(false);

// ========================================
// AUTH GATES
// ========================================

/// Flipped to `true` whenever an API call returns 401 Unauthorized. App.svelte
/// watches this and swaps the view to the Login page; the login flow resets
/// it on success.
export const authRequired = writable<boolean>(false);

/// Flipped to `true` whenever an API call returns 503 + setup_required. The
/// setup wizard resets it once the user completes onboarding.
export const setupRequired = writable<boolean>(false);

// ========================================
// SIDEBAR COLLAPSED
// ========================================

function createSidebarCollapsedStore() {
  const stored = typeof localStorage !== "undefined" ? localStorage.getItem("sidebar-collapsed") : null;
  const initial = stored === "true";
  const store = writable<boolean>(initial);
  const { subscribe, set } = store;
  return {
    subscribe,
    set(value: boolean) {
      if (typeof localStorage !== "undefined") localStorage.setItem("sidebar-collapsed", String(value));
      set(value);
    },
    toggle() {
      const next = !get(store);
      if (typeof localStorage !== "undefined") localStorage.setItem("sidebar-collapsed", String(next));
      set(next);
    },
  };
}

export const sidebarCollapsed = createSidebarCollapsedStore();

// ========================================
// FAVICON BADGE
// ========================================

let originalFavicon: string | null = null;

export function updateFaviconBadge(count: number) {
  if (typeof document === "undefined") return;
  const link = document.querySelector("link[rel~='icon']") as HTMLLinkElement | null;
  if (!link) return;

  if (!originalFavicon) originalFavicon = link.href;

  if (count <= 0) {
    link.href = originalFavicon;
    return;
  }

  const img = new Image();
  img.crossOrigin = "anonymous";
  img.onload = () => {
    const canvas = document.createElement("canvas");
    canvas.width = 32;
    canvas.height = 32;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.drawImage(img, 0, 0, 32, 32);
    // Red badge circle
    ctx.fillStyle = "#ef4444";
    ctx.beginPath();
    ctx.arc(24, 8, 8, 0, 2 * Math.PI);
    ctx.fill();
    // Count text
    ctx.fillStyle = "white";
    ctx.font = "bold 10px sans-serif";
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    ctx.fillText(count > 9 ? "9+" : String(count), 24, 8);
    link.href = canvas.toDataURL();
  };
  img.src = originalFavicon;
}

// Update favicon when stats change
if (typeof window !== "undefined") {
  stats.subscribe((s) => updateFaviconBadge(s.active_downloads));
}

// ========================================
// aria-live ANNOUNCER
// ========================================

export const ariaAnnouncement = writable<string>("");
