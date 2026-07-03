import { Controller } from "@hotwired/stimulus"

// Drives the /calendar management sidebar: one header button that collapses the
// desktop aside (persisted, mirrors folder-pane) or opens the mobile <dialog>,
// auto-submit for the sidebar's color pickers, and outside-click dismissal of
// the per-calendar "…" menus. Lives on the calendar page root next to
// calendar-nav; uses no keyboard shortcuts (those belong to calendar-nav).
export default class extends Controller {
  static targets = ["aside", "dialog", "menu"]
  static values = {
    storageKey: { type: String, default: "campbooks:calendar-sidebar-collapsed" },
    breakpoint: { type: Number, default: 1024 } // Tailwind lg — below it the aside is hidden
  }

  connect() {
    if (this.#stored() === "1") this.#applyCollapsed(true)
  }

  // The header button: opens the dialog below lg, toggles the aside at lg+.
  toggle() {
    if (window.innerWidth < this.breakpointValue) {
      if (this.hasDialogTarget) this.dialogTarget.showModal()
    } else {
      const collapsed = !this.#collapsed()
      this.#applyCollapsed(collapsed)
      this.#persist(collapsed)
    }
  }

  closeDialog() {
    if (this.hasDialogTarget) this.dialogTarget.close()
  }

  // A click on the <dialog> element itself (not its contents) is the backdrop.
  backdropClose(event) {
    if (event.target === this.dialogTarget) this.closeDialog()
  }

  // change->calendar-sidebar#submit on the color-picker forms: picking a swatch
  // saves immediately (the radios have no submit button).
  submit(event) {
    event.target.form?.requestSubmit()
  }

  // click@window: close any open "…" calendar menu the click landed outside of.
  closeMenus(event) {
    this.menuTargets.forEach((menu) => {
      if (menu.open && !menu.contains(event.target)) menu.open = false
    })
  }

  #collapsed() {
    return this.hasAsideTarget && this.asideTarget.classList.contains("lg:hidden")
  }

  #applyCollapsed(collapsed) {
    if (!this.hasAsideTarget) return
    this.asideTarget.classList.toggle("lg:hidden", collapsed)
    this.asideTarget.classList.toggle("lg:flex", !collapsed)
  }

  #persist(collapsed) {
    try { localStorage.setItem(this.storageKeyValue, collapsed ? "1" : "0") } catch (_) { /* private mode */ }
  }

  #stored() {
    try { return localStorage.getItem(this.storageKeyValue) } catch (_) { return null }
  }
}
