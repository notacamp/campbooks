import { Controller } from "@hotwired/stimulus"

// Mobile master-detail toggle for the email show page. The desktop layout shows
// the thread list and the email detail side by side; on mobile only one fits, so
// this swaps between them. Default state is "detail" (you arrived by opening a
// message); the in-detail back button calls showList(). Desktop is never touched —
// the toggle buttons are sm:hidden, and any inline overrides are cleared at >=sm.
export default class extends Controller {
  static targets = ["list", "detail"]
  // Width (px) at/above which both panes show side by side and inline overrides
  // should be cleared. Email = 1024 (lg), Scout chat = 640 (sm, the default).
  static values = { breakpoint: { type: Number, default: 640 } }

  connect() {
    this.boundResize = this.#onResize.bind(this)
    window.addEventListener("resize", this.boundResize)
  }

  disconnect() {
    window.removeEventListener("resize", this.boundResize)
  }

  showList() {
    if (this.hasListTarget) this.listTarget.style.display = "flex"
    if (this.hasDetailTarget) this.detailTarget.style.display = "none"
  }

  showDetail() {
    this.#reset()
  }

  #reset() {
    if (this.hasListTarget) this.listTarget.style.display = ""
    if (this.hasDetailTarget) this.detailTarget.style.display = ""
  }

  #onResize() {
    if (window.innerWidth >= this.breakpointValue) this.#reset()
  }
}
