// The desktop app's update choice and self-update in the web UI (spec
// `desktop-app`, AC-18, AC-25 to AC-30). Every decision is a pure function
// here so it is unit-tested; the components only render the result.
//
// Desktop mode is detected by `GET /api/v1/updates/mode`: it answers 200 in
// the desktop app and 404 against `krake-server` or Docker, where `null`
// stands for "no desktop mode" and nothing of this shows.

import type { CoreApplyResult, DesktopModeState, UpdateCheck, UpdateMode } from "./api";

/** The three choices, in the order the question and About show them. */
export const UPDATE_MODES: readonly UpdateMode[] = ["auto", "notify", "off"];

/** Preselected in the first-start question (AC-25). */
export const DEFAULT_UPDATE_MODE: UpdateMode = "auto";

/** How often the open window asks for the question / an announcement. */
export const MODE_POLL_MS = 60_000;

/**
 * Whether the first-start question shows (AC-25, AC-26): only in the desktop
 * app, only for a copy that can update itself, only while the server says no
 * choice is stored and it was not closed, and never again once this window
 * confirmed or closed it.
 */
export function shouldAskQuestion(state: DesktopModeState | null, handledThisRun: boolean): boolean {
  if (!state || handledThisRun) return false;
  return state.can_self_update && state.question_pending;
}

/**
 * The version the "notify" announcement shows, or null (AC-28, AC-29).
 *
 * The server only reports an announcement under "notify" and forgets it once
 * it is marked; `handled` holds the versions this window already answered
 * (Later, close, a successful Install), so a poll that races the mark does
 * not bring it back. The first-start question goes first.
 */
export function announcementToShow(
  state: DesktopModeState | null,
  handled: ReadonlySet<string>,
  questionOpen: boolean,
): string | null {
  if (!state || questionOpen) return null;
  if (!state.can_self_update || state.update_mode === "off") return null;
  const version = state.announcement;
  if (!version || handled.has(version)) return null;
  return version;
}

/** What the About section shows besides the running version (AC-30). */
export type AboutView =
  /** Browser against `krake-server` / Docker: the version only. */
  | { kind: "version" }
  /** Desktop copy that cannot update itself (`.deb` / `.rpm`, debug): the reason. */
  | { kind: "package-manager" }
  /** Desktop copy that updates itself: pending version, choices, check button. */
  | { kind: "desktop"; mode: UpdateMode; pending: string | null };

export function aboutView(state: DesktopModeState | null): AboutView {
  if (!state) return { kind: "version" };
  if (!state.can_self_update) return { kind: "package-manager" };
  return { kind: "desktop", mode: state.update_mode, pending: state.pending_restart ?? null };
}

/** The core-update banner on the Plugins page (AC-18). */
export type BannerView =
  | { kind: "hidden" }
  /** An update with an Update button. */
  | { kind: "offer"; from: string; to: string }
  /** An update this copy cannot install itself: a hint instead of the button. */
  | { kind: "package-manager"; from: string; to: string }
  /** Downloaded; takes effect at the next start. */
  | { kind: "ready"; version: string };

/**
 * `ready` is the version a successful apply in this view returned; a
 * `pending_restart` from the check shows the same text on load.
 */
export function bannerView(check: UpdateCheck | null, ready: string | null): BannerView {
  const core = check?.core;
  const pending = ready ?? core?.pending_restart ?? null;
  if (pending) return { kind: "ready", version: pending };
  if (!core?.update_available) return { kind: "hidden" };
  const from = core.current_version;
  const to = core.latest_version ?? "";
  if (packageManaged(core)) return { kind: "package-manager", from, to };
  return { kind: "offer", from, to };
}

/**
 * A desktop copy that may not replace itself (`.deb` / `.rpm`, debug build,
 * AC-16). Only the desktop app says so: a plain server reports
 * `can_self_update: false` for Docker too, where the banner keeps its button.
 */
export function packageManaged(core: UpdateCheck["core"] | undefined): boolean {
  return core?.distribution?.toLowerCase() === "tauri" && core.can_self_update === false;
}

/** The version of a desktop apply's `{"status":"ready","version":"…"}`, else null. */
export function readyVersion(result: CoreApplyResult | undefined | null): string | null {
  if (!result || result.status !== "ready") return null;
  return typeof result.version === "string" && result.version ? result.version : null;
}

/** The outcome of About's Check for updates (AC-30). */
export type CheckOutcome =
  | { kind: "current" }
  | { kind: "package-manager" }
  /** Desktop apply: ready at the next start. */
  | { kind: "ready"; version: string }
  /** Server apply (no desktop answer): started, needs a restart. */
  | { kind: "started" };

/**
 * Check, then apply when there is something to install — the same two calls
 * as the banner, whatever the update choice. Errors propagate to the caller.
 */
export async function checkAndApply(
  check: () => Promise<UpdateCheck>,
  apply: () => Promise<CoreApplyResult | undefined>,
): Promise<CheckOutcome> {
  const { core } = await check();
  if (core?.pending_restart && !core.update_available) {
    return { kind: "ready", version: core.pending_restart };
  }
  if (!core?.update_available) return { kind: "current" };
  if (packageManaged(core)) return { kind: "package-manager" };
  const version = readyVersion(await apply());
  return version ? { kind: "ready", version } : { kind: "started" };
}
