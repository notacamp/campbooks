/** A ref callback that puts the focus in a field when it appears (a step that replaces another inside a menu). */
export function focusOnMount(node: HTMLInputElement | null): void {
  node?.focus();
}
