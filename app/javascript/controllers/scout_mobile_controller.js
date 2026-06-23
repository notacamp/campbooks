import { Controller } from "@hotwired/stimulus"

// On small screens the Scout chat panel is hidden inline (it lives in the desktop
// split layout). This surfaces it as a full-screen overlay when the mobile
// "Scout" button is tapped. We drive it with inline styles rather than classes so
// it reliably overrides the panel's own `relative`/`w-96`/`sm:flex` utilities and
// any inline width set by panel-resize. Desktop is untouched — open/close are only
// wired to sm:hidden buttons, and inline styles are cleared on close.
export default class extends Controller {
  static targets = ["panel", "backdrop"]

  // Arriving with #discussion on a small screen (the drawer's "Discussion" button
  // opening the full view) surfaces the overlay straight away. Desktop ignores
  // this — chat-panel handles the #discussion case there.
  connect() {
    if (window.location.hash === "#discussion" && window.innerWidth < 1024) {
      this.open()
    }
  }

  OVERLAY = {
    position: "fixed",
    top: "3.5rem", // below the 56px topbar
    left: "0",
    right: "0",
    bottom: "0",
    width: "100%",
    zIndex: "50",
    display: "flex"
  }

  open() {
    if (!this.hasPanelTarget) return
    this.panelTarget.classList.remove("hidden")
    Object.assign(this.panelTarget.style, this.OVERLAY)
    this.backdropTarget?.classList.remove("hidden")
    document.body.style.overflow = "hidden"
  }

  close() {
    if (!this.hasPanelTarget) return
    this.panelTarget.classList.add("hidden")
    Object.keys(this.OVERLAY).forEach((prop) => { this.panelTarget.style[prop] = "" })
    this.backdropTarget?.classList.add("hidden")
    document.body.style.overflow = ""
  }
}
