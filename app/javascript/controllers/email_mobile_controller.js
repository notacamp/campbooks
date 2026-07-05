import { Controller } from "@hotwired/stimulus"

// Mobile master-detail toggle for the email show page. The desktop layout shows
// the thread list and the email detail side by side; on mobile only one fits, so
// this swaps between them. Default state is "detail" (you arrived by opening a
// message); the in-detail back button calls showList(). Desktop is never touched —
// the toggle buttons are sm:hidden, and any inline overrides are cleared at >=sm.
export default class extends Controller {
  // `topband` is the content-spanning skim + search + compose strip. On mobile it
  // belongs to the list view (hidden while reading a message); on desktop it always
  // shows (CSS `lg:flex`, and #onResize clears the inline override at the breakpoint).
  static targets = ["list", "detail", "topband"]
  // Width (px) at/above which both panes show side by side and inline overrides
  // should be cleared. Email = 1024 (lg), Scout chat = 640 (sm, the default).
  static values = { breakpoint: { type: Number, default: 640 } }

  connect() {
    this.boundResize = this.#onResize.bind(this)
    window.addEventListener("resize", this.boundResize)
    // The server always 302-redirects /email_messages to the latest email. When
    // the nav item or a folder chip sends show_list=1, land on the list instead
    // of the detail pane on mobile. Skipping the param means a direct deep-link
    // (push notification, digest link) still opens the email as expected.
    // frameLoad() flips back to detail after a row tap loads the email_detail frame.
    const showList = new URLSearchParams(window.location.search).get("show_list") === "1"
    if (window.innerWidth < this.breakpointValue && showList) this.showList()
  }

  disconnect() {
    window.removeEventListener("resize", this.boundResize)
  }

  showList() {
    if (this.hasListTarget) this.listTarget.style.display = "flex"
    if (this.hasDetailTarget) this.detailTarget.style.display = "none"
    if (this.hasTopbandTarget) this.topbandTarget.style.display = "flex"
  }

  showDetail() {
    this.#reset()
  }

  // Wired on the email_detail turbo frame (email show shell): after an in-frame
  // email navigation — a row tap from the revealed list on mobile — flip back to
  // the detail pane. Desktop is unaffected (#reset only clears inline overrides).
  frameLoad(event) {
    if (event.target && event.target.id === "email_detail") this.showDetail()
  }

  #reset() {
    if (this.hasListTarget) this.listTarget.style.display = ""
    if (this.hasDetailTarget) this.detailTarget.style.display = ""
    // Clear the inline override so the band reverts to its class state
    // (hidden on mobile detail, lg:flex on desktop).
    if (this.hasTopbandTarget) this.topbandTarget.style.display = ""
  }

  #onResize() {
    if (window.innerWidth >= this.breakpointValue) this.#reset()
  }
}
