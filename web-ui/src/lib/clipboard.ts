// Copy text to the clipboard.
//
// `navigator.clipboard` only exists in secure contexts (HTTPS or localhost),
// and Krake is often reached over plain HTTP on the LAN (`http://nas:1516`).
// There the legacy copy command on a selected, off-screen textarea still works.

/** Copies `text`; rejects when the browser refuses. */
export async function copyText(text: string): Promise<void> {
  if (navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(text);
    return;
  }
  const field = document.createElement("textarea");
  field.value = text;
  field.setAttribute("readonly", "");
  field.style.position = "fixed";
  field.style.opacity = "0";
  field.style.pointerEvents = "none";
  const previous = document.activeElement as HTMLElement | null;
  document.body.append(field);
  try {
    field.focus();
    field.select();
    if (!document.execCommand("copy")) throw new Error("copy refused");
  } finally {
    field.remove();
    previous?.focus?.();
  }
}
