// Wires the link list import (linkListImport.ts) to the API, the download
// stores and the toasts. Used by the drop zone, the paste handler and the
// add panel (spec `link-list-import`).

import { get } from "svelte/store";
import { addBatch } from "./api";
import { downloads, usenetDownloads } from "./stores";
import { addToast } from "./toast";
import { locale, tr, type TParams } from "./i18n";
import {
  MAX_LIST_BYTES,
  prepareLinkList,
  problemMessage,
  queueLinkList,
  resultToast,
  type ActiveDownloads,
  type ImportResult,
} from "./linkListImport";
import type { LinkList } from "./linkList";

export { chooseTextFile } from "./linkListImport";

const ACTIVE_STATUS = new Set(["queued", "downloading", "paused"]);

/** URLs and ids of the downloads in the list that are still active. */
export function activeDownloads(): ActiveDownloads {
  const all = [...get(downloads), ...get(usenetDownloads)].filter((d) => ACTIVE_STATUS.has(d.status));
  return { urls: new Set(all.map((d) => d.url)), ids: new Set(all.map((d) => d.id)) };
}

export function translator() {
  const lang = get(locale);
  return (key: string, params?: TParams) => tr(lang, key, params);
}

/** The message for a list that is too large or holds no link. */
export function listProblem(kind: "too_large" | "no_links", name?: string): string {
  return problemMessage(kind, name, translator());
}

/** Queue a parsed list and report it in one toast. */
export async function queueAndReport(list: LinkList): Promise<ImportResult> {
  const result = await queueLinkList(list, activeDownloads(), addBatch);
  const toast = resultToast(result, translator());
  addToast(toast.type, toast.title, toast.message, {
    multiline: toast.multiline,
    ...(toast.sticky ? { duration: 0 } : {}),
  });
  return result;
}

/** Import a dropped or pasted link list: size check, parse, queue, toast. */
export async function importLinkList(input: Uint8Array | string, name?: string, sizeBytes?: number) {
  const prepared = prepareLinkList(input, sizeBytes);
  if (prepared.kind !== "ok") {
    addToast("error", listProblem(prepared.kind, name));
    return;
  }
  await queueAndReport(prepared.list);
}

/** Import a dropped file without reading one that is over the limit. */
export async function importLinkListFile(file: File) {
  if (file.size > MAX_LIST_BYTES) {
    addToast("error", listProblem("too_large", file.name));
    return;
  }
  await importLinkList(new Uint8Array(await file.arrayBuffer()), file.name, file.size);
}
