import { Controller } from "@hotwired/stimulus"

export default class extends Controller {
  static targets = ["sidebar", "collapsedToggle", "folders"]

  connect() {
    // The sidebar renders collapsed (`hidden`) by default, so derive the initial
    // state from the DOM. Hard-coding `false` desynced state from markup and made
    // the first "Expand" click a no-op — you had to click twice to open it.
    this.collapsed = this.hasSidebarTarget && this.sidebarTarget.classList.contains("hidden")
  }

  toggle() {
    this.collapsed = !this.collapsed
    if (this.collapsed) {
      this.sidebarTarget.classList.add("hidden")
      this.collapsedToggleTarget.classList.remove("hidden")
      this.collapsedToggleTarget.classList.add("flex")
    } else {
      this.sidebarTarget.classList.remove("hidden")
      this.collapsedToggleTarget.classList.add("hidden")
      this.collapsedToggleTarget.classList.remove("flex")
    }
  }
}
