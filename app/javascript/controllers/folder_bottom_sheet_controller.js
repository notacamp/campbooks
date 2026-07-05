import { Controller } from "@hotwired/stimulus"

// Manages the mobile folder bottom sheet: open / close animations, backdrop,
// focus-trap, and Escape close. The sheet is a fixed overlay that slides up
// from the bottom; the chip bar's "Folders" trigger fires `open`.
//
// HTML contract:
//   data-controller="folder-bottom-sheet"
//   data-folder-bottom-sheet-target="backdrop"   ← fixed backdrop div
//   data-folder-bottom-sheet-target="panel"      ← slide-up sheet panel
//   data-action="folder-bottom-sheet#open"       ← on the trigger button
export default class extends Controller {
  static targets = ["backdrop", "panel"]

  connect() {
    this._boundKeydown = this._keydown.bind(this)
    document.addEventListener("keydown", this._boundKeydown)
  }

  disconnect() {
    document.removeEventListener("keydown", this._boundKeydown)
  }

  open() {
    // Remove invisible so the panel is in the paint tree, then flush (reflow)
    // so the translate-y-full state is painted before we transition away from it.
    this.panelTarget.classList.remove("invisible")
    void this.panelTarget.offsetHeight

    this.panelTarget.classList.remove("translate-y-full")
    this.panelTarget.classList.add("translate-y-0")

    this.backdropTarget.style.display = ""

    // Focus the first interactive element inside the panel
    const first = this.panelTarget.querySelector(
      "a[href], button:not([disabled]), input, select, textarea"
    )
    first?.focus()
  }

  close() {
    this.panelTarget.classList.remove("translate-y-0")
    this.panelTarget.classList.add("translate-y-full")

    this.backdropTarget.style.display = "none"

    // After the slide-down animation (duration-300) fully hide the panel so
    // it doesn't leave a sliver peeking above the viewport bottom.
    clearTimeout(this._hideTimeout)
    this._hideTimeout = setTimeout(() => {
      this.panelTarget.classList.add("invisible")
    }, 300)
  }

  // ── Tab focus trap ───────────────────────────────────────────
  trapTab(event) {
    if (event.key !== "Tab") return
    const focusables = Array.from(
      this.panelTarget.querySelectorAll(
        "a[href], button:not([disabled]), input:not([type='hidden']):not([disabled]), select, textarea, [contenteditable='true']"
      )
    )
    if (!focusables.length) return
    const first = focusables[0]
    const last = focusables[focusables.length - 1]
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault()
      last.focus()
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault()
      first.focus()
    }
  }

  // ── Private ──────────────────────────────────────────────────
  _keydown(event) {
    if (event.key === "Escape" && !this.panelTarget.classList.contains("invisible")) {
      this.close()
    }
    if (event.key === "Tab" && !this.panelTarget.classList.contains("invisible")) {
      this.trapTab(event)
    }
  }
}
