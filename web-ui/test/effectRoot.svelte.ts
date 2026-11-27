// Runs `fn` inside an effect root, so code that registers `$effect` (the
// download action controller) can be called from a plain test file.
export function inEffectRoot<T>(fn: () => T): { value: T; dispose: () => void } {
  let value!: T;
  const dispose = $effect.root(() => {
    value = fn();
  });
  return { value, dispose };
}
