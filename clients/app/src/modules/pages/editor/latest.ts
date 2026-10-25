/**
 * A value read later, from callbacks that outlive a render (the editor's
 * menus are built once): `set` it in an effect, read `current` in the
 * callback. Not a React ref, so nothing here is read while rendering.
 */
export class Latest<T> {
  #value: T;

  constructor(initial: T) {
    this.#value = initial;
  }

  get current(): T {
    return this.#value;
  }

  set(next: T): void {
    this.#value = next;
  }
}
