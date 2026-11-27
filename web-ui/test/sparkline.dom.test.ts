// @vitest-environment happy-dom
import { describe, it, expect, afterEach, vi } from "vitest";
import { mount, unmount } from "svelte";
import Sparkline from "@amigo-labs/krake-ui/components/Sparkline.svelte";

// Over plain HTTP on a host other than localhost (e.g. `http://nas:1516`) the
// page is not a secure context and `crypto.randomUUID` does not exist. The
// sidebar mounts the sparkline when it expands; a throw there aborted the
// update and left the sidebar half expanded.

const apps: ReturnType<typeof mount>[] = [];

afterEach(() => {
  for (const app of apps.splice(0)) unmount(app);
  vi.unstubAllGlobals();
  document.body.innerHTML = "";
});

function render() {
  const target = document.createElement("div");
  document.body.append(target);
  apps.push(mount(Sparkline, { target, props: { values: [1, 2, 3] } }));
  return target;
}

describe("Sparkline", () => {
  it("renders outside a secure context, where crypto.randomUUID is missing", () => {
    vi.stubGlobal("crypto", { getRandomValues: crypto.getRandomValues.bind(crypto) });
    const target = render();
    expect(target.querySelector("polyline")).not.toBeNull();
  });

  it("gives every instance its own gradient id", () => {
    const a = render().querySelector("linearGradient")?.id;
    const b = render().querySelector("linearGradient")?.id;
    expect(a).toBeTruthy();
    expect(a).not.toBe(b);
  });
});
