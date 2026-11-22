// The add panel's plugin suggestions (spec plugin-suggest-on-add): which
// registry plugin handles the pasted links better than the installed ones.

/** One suggestion of `POST /plugins/suggest` with `{urls}` (AC-01). */
export interface LinkSuggestion {
  id: string;
  name: string;
  version: string;
  /** `null`: unscoped, may reach any public host; `[]`: no host at all. */
  domains: string[] | null;
  /** The covered links, each once, in input order. */
  urls: string[];
  action: "install" | "enable";
}

/** Quiet time of the URL field before a lookup (AC-10). */
export const SUGGEST_DELAY_MS = 500;

export interface SuggestionLookup {
  /** The field's links changed. */
  update(links: string[]): void;
  /** Ask again at once for the current links (after an install, AC-11). */
  refresh(): void;
  dispose(): void;
}

/**
 * Debounced lookup that only reports the answer to its latest request
 * (AC-14). A failed request reports nothing (AC-13); a field without links
 * reports an empty list at once.
 */
export function createSuggestionLookup(opts: {
  fetch: (urls: string[]) => Promise<LinkSuggestion[]>;
  onResult: (suggestions: LinkSuggestion[]) => void;
  delayMs?: number;
}): SuggestionLookup {
  const delay = opts.delayMs ?? SUGGEST_DELAY_MS;
  let links: string[] = [];
  let timer: ReturnType<typeof setTimeout> | null = null;
  // Bumped on every change: an answer carrying an older number is dropped.
  let generation = 0;

  function cancel() {
    if (timer !== null) clearTimeout(timer);
    timer = null;
  }

  /** `refresh`: a failure keeps the cards; a changed field: it clears them. */
  function run(refresh = false) {
    cancel();
    const mine = ++generation;
    if (links.length === 0) {
      opts.onResult([]);
      return;
    }
    opts.fetch(links).then(
      (suggestions) => {
        if (mine === generation) opts.onResult(suggestions);
      },
      () => {
        // No error for a failed lookup: adding links never depends on it.
        // Cards of links that left the field go, though (AC-14).
        if (mine === generation && !refresh) opts.onResult([]);
      },
    );
  }

  return {
    update(next) {
      cancel();
      links = next;
      if (next.length === 0) {
        run();
        return;
      }
      generation++;
      timer = setTimeout(() => run(), delay);
    },
    refresh: () => run(true),
    dispose() {
      cancel();
      generation++;
    },
  };
}

/** How a card shows a plugin's domains (AC-10). */
export type DomainView =
  | { kind: "any" }
  | { kind: "none" }
  | { kind: "list"; shown: string[]; hidden: string[] };

/** Domains shown before the "show all" toggle. */
const SHOWN_DOMAINS = 3;

export function domainView(domains: string[] | null): DomainView {
  if (domains === null) return { kind: "any" };
  if (domains.length === 0) return { kind: "none" };
  return { kind: "list", shown: domains.slice(0, SHOWN_DOMAINS), hidden: domains.slice(SHOWN_DOMAINS) };
}
