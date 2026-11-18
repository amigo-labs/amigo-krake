// Queueing a link list from the web UI (spec `link-list-import`, AC-05 to
// AC-11): size limit, chunked batch requests, result counts and the text of
// the result toast. Kept free of Svelte so it is unit-tested.

import { parseLinkList, type LinkList } from "./linkList";

/** Largest link list the web UI accepts (bytes as UTF-8, or file size). */
export const MAX_LIST_BYTES = 1024 * 1024;

export type PreparedList =
  | { kind: "too_large" }
  | { kind: "no_links"; notRecognized: number }
  | { kind: "ok"; list: LinkList };

const encoder = new TextEncoder();

/**
 * Check the size limit and parse. `sizeBytes` is a file's size on disk; for
 * text it defaults to its length in UTF-8 bytes.
 */
export function prepareLinkList(input: Uint8Array | string, sizeBytes?: number): PreparedList {
  const size =
    sizeBytes ?? (typeof input === "string" ? encoder.encode(input).length : input.length);
  if (size > MAX_LIST_BYTES) return { kind: "too_large" };
  const list = parseLinkList(input);
  if (list.links.length === 0) return { kind: "no_links", notRecognized: list.notRecognized };
  return { kind: "ok", list };
}

export interface ImportResult {
  queued: number;
  alreadyQueued: number;
  rejected: { url: string; reason: string }[];
  notRecognized: number;
  /** Links not sent because a request failed as a whole. */
  notSent: number;
  /** Message of the request that failed as a whole, if any. */
  failure?: string;
}

/** What the server knows already: URLs and ids of active downloads. */
export interface ActiveDownloads {
  urls: Set<string>;
  ids: Set<string>;
}

export type SendBatch = (urls: string[]) => Promise<{ ids: string[]; errors?: string[] }>;

/** Largest JSON body per batch request, well below the server's 2 MiB. */
const MAX_BODY_BYTES = 1024 * 1024;

/** Split links into batches whose `{"urls": [...]}` body fits the limit. */
export function chunkLinks(links: string[], maxBodyBytes = MAX_BODY_BYTES): string[][] {
  const overhead = encoder.encode('{"urls":[]}').length;
  const chunks: string[][] = [];
  let current: string[] = [];
  let size = overhead;
  for (const link of links) {
    // The quoted, escaped link plus a separating comma.
    const cost = encoder.encode(JSON.stringify(link)).length + 1;
    if (current.length > 0 && size + cost > maxBodyBytes) {
      chunks.push(current);
      current = [];
      size = overhead;
    }
    current.push(link);
    size += cost;
  }
  if (current.length > 0) chunks.push(current);
  return chunks;
}

/**
 * Send the links of a list that are not active downloads already, batch by
 * batch. A batch that fails as a whole stops the import: what was sent
 * before stays queued, the rest is counted as not sent.
 */
export async function queueLinkList(
  list: LinkList,
  active: ActiveDownloads,
  send: SendBatch,
): Promise<ImportResult> {
  const fresh = list.links.filter((l) => !active.urls.has(l));
  const result: ImportResult = {
    queued: 0,
    alreadyQueued: list.links.length - fresh.length,
    rejected: [],
    notRecognized: list.notRecognized,
    notSent: 0,
  };
  let sent = 0;
  for (const chunk of chunkLinks(fresh)) {
    let response: { ids: string[]; errors?: string[] };
    try {
      response = await send(chunk);
    } catch (e) {
      result.notSent = fresh.length - sent;
      result.failure = e instanceof Error ? e.message : String(e);
      break;
    }
    sent += chunk.length;
    // The server reports a rejected link as "<url>: <reason>" and returns an
    // id, in order, for every other link of the batch.
    const reasons = new Map<string, string>();
    for (const err of response.errors ?? []) {
      const url = chunk
        .filter((u) => err.startsWith(`${u}: `))
        .sort((a, b) => b.length - a.length)[0];
      if (url !== undefined) reasons.set(url, err.slice(url.length + 2));
    }
    let next = 0;
    for (const url of chunk) {
      const reason = reasons.get(url);
      if (reason !== undefined) {
        result.rejected.push({ url, reason });
        continue;
      }
      const id = response.ids[next++];
      if (id !== undefined && active.ids.has(id)) result.alreadyQueued += 1;
      else result.queued += 1;
    }
  }
  return result;
}

type Translate = (key: string, params?: Record<string, string | number>) => string;

export interface ResultToast {
  type: "success" | "error" | "info";
  title: string;
  message?: string;
}

/** How many rejected links the toast names before "and N more". */
const SHOWN_REJECTED = 5;

/** The one toast that reports an import: counts that are not zero, the
 * first rejected links with their reason, and a failure if one stopped it. */
export function resultToast(result: ImportResult, t: Translate): ResultToast {
  const counts: [string, number][] = [
    ["linklist.queued", result.queued],
    ["linklist.already_queued", result.alreadyQueued],
    ["linklist.rejected", result.rejected.length],
    ["linklist.not_recognized", result.notRecognized],
  ];
  const title = counts
    .filter(([, n]) => n > 0)
    .map(([key, n]) => t(key, { count: n }))
    .join(" · ");

  const lines = result.rejected.slice(0, SHOWN_REJECTED).map((r) => `${r.url} — ${r.reason}`);
  if (result.rejected.length > SHOWN_REJECTED) {
    lines.push(t("linklist.more", { count: result.rejected.length - SHOWN_REJECTED }));
  }
  if (result.notSent > 0) {
    lines.push(t("linklist.not_sent", { count: result.notSent, reason: result.failure ?? "" }));
  }

  const nothingQueued = result.queued + result.alreadyQueued === 0;
  const type = result.notSent > 0 || nothingQueued ? "error" : result.rejected.length > 0 ? "info" : "success";
  return { type, title, message: lines.length > 0 ? lines.join("\n") : undefined };
}

// ---- Component wiring, kept here so it is unit-tested ----

export type DropRoute = "dlc" | "nzb" | "link_list";

/** How a dropped file is imported: DLC and NZB by extension, anything
 * else as a link list (AC-05). */
export function routeDroppedFile(name: string): DropRoute {
  const ext = name.split(".").pop()?.toLowerCase();
  return ext === "dlc" ? "dlc" : ext === "nzb" ? "nzb" : "link_list";
}

const TEXT_FIELD = "input, textarea, select, [contenteditable]:not([contenteditable='false'])";

/** Whether an event target is, or sits inside, a field that takes text. */
export function inTextField(target: unknown): boolean {
  const closest = (target as { closest?: unknown } | null)?.closest;
  return typeof closest === "function" && closest.call(target, TEXT_FIELD) !== null;
}

/** A paste imports only while no text field has it or has focus (AC-07). */
export function shouldImportPaste(target: unknown, activeElement: unknown): boolean {
  return !inTextField(target) && !inTextField(activeElement);
}

/** The message for a list that is too large or holds no link (AC-09, AC-10). */
export function problemMessage(
  kind: "too_large" | "no_links",
  name: string | undefined,
  t: Translate,
): string {
  const subject = name ?? t("linklist.the_text");
  return kind === "too_large"
    ? t("linklist.too_large", { name: subject, limit: "1 MiB" })
    : t("linklist.no_links", { name: subject });
}

export interface FileChoice {
  /** The add panel's URL field after the choice. */
  urlInput: string;
  /** Lines of the file that were not links. */
  fileSkipped: number;
  /** Why the field was left as it was, if it was. */
  problem: string | null;
}

/** The add panel after the user chose a text file (AC-06): the field holds
 * the links, or keeps its content and a problem is named. */
export function chooseTextFile(
  prepared: PreparedList,
  current: string,
  name: string,
  t: Translate,
): FileChoice {
  if (prepared.kind === "ok") {
    return {
      urlInput: prepared.list.links.join("\n"),
      fileSkipped: prepared.list.notRecognized,
      problem: null,
    };
  }
  return { urlInput: current, fileSkipped: 0, problem: problemMessage(prepared.kind, name, t) };
}

/** Whether a drag that carries text is imported: plain text from outside
 * the app (not files, not a queue reorder), not dropped on a text field
 * (AC-07). */
export function shouldImportTextDrop(
  types: readonly string[],
  drag: { files: boolean; internal: boolean },
  target: unknown,
): boolean {
  return types.includes("text/plain") && !drag.files && !drag.internal && !inTextField(target);
}

/** The text of a paste to import, or `null` when the paste belongs to a
 * field or carries no text (AC-07). */
export function pastedText(text: string | undefined, target: unknown, activeElement: unknown): string | null {
  if (!text || !shouldImportPaste(target, activeElement)) return null;
  return text;
}
