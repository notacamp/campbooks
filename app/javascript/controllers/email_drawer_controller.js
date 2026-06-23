import { Controller } from "@hotwired/stimulus"

export default class extends Controller {
  static targets = ["backdrop", "panel"]

  connect() {
    this.boundKeydown = this._keydown.bind(this)
    this.boundBeforeVisit = this.close.bind(this)
    this.boundClick = this._interceptClick.bind(this)
    document.addEventListener("keydown", this.boundKeydown)
    document.addEventListener("turbo:before-visit", this.boundBeforeVisit)
    // Use capture phase so our handler fires before Turbo's own capture-phase
    // click listener, giving us a chance to preventDefault before turbo:visit
    // starts for data-turbo-frame="_top" links in List/Board layout.
    document.addEventListener("click", this.boundClick, true)

    // The frame ships with a spinner placeholder. Remember it so closing can
    // restore it — dropping the loaded email's HTML from the DOM — and so the
    // next open shows the spinner again instead of the stale previous message.
    const frame = this.element.querySelector("turbo-frame")
    this.framePlaceholder = frame ? frame.innerHTML : ""
  }

  disconnect() {
    document.removeEventListener("keydown", this.boundKeydown)
    document.removeEventListener("turbo:before-visit", this.boundBeforeVisit)
    document.removeEventListener("click", this.boundClick, true)
    clearTimeout(this.hideTimeout)
  }

  open({ params: { messageId } }) {
    if (!messageId) return
    this._loadMessage(messageId)
  }

  close() {
    this.panelTarget.classList.add("translate-y-full")
    this.panelTarget.classList.remove("translate-y-0")
    this.backdropTarget.style.display = "none"

    // After the slide-down animation (duration-300) finishes: fully hide the panel
    // — translate-y-full alone leaves a sliver peeking above the viewport bottom on
    // desktop — and restore the spinner placeholder so the email's HTML actually
    // leaves the page (and a reopen refetches, since src is cleared too).
    clearTimeout(this.hideTimeout)
    this.hideTimeout = setTimeout(() => {
      this.panelTarget.classList.add("invisible")
      const frame = this.element.querySelector("turbo-frame")
      if (frame) {
        frame.removeAttribute("src")
        frame.innerHTML = this.framePlaceholder
      }
    }, 300)
  }

  _interceptClick(event) {
    const link = event.target.closest("a[href]")
    if (!link) return

    const url = link.getAttribute("href")
    if (!url) return

    const match = url.match(/\/email_messages\/(\d+)(\?.*)?$/)
    if (!match) return

    if (event.metaKey || event.ctrlKey || event.shiftKey || event.button !== 0) return
    // Inbox rows carry data-turbo-frame="_top" so they navigate the full page in
    // the Default layout. In the List/Board layouts the same rows should open
    // this drawer instead.
    if (link.getAttribute("data-turbo-frame") === "_top" && !this._inboxUsesDrawer()) return

    event.preventDefault()
    this._loadMessage(match[1])
  }

  // True when the inbox is in a layout where clicking a row should open the
  // drawer rather than navigate the page (List or Board).
  _inboxUsesDrawer() {
    const layout = document.querySelector("[data-inbox-layout]")?.getAttribute("data-inbox-layout")
    return layout === "list" || layout === "board"
  }

  _loadMessage(messageId) {
    // A pending close() may be about to hide the panel / empty the frame — cancel it.
    clearTimeout(this.hideTimeout)

    // Reveal the panel, then flush styles (reflow) so the slide-up animates from
    // translate-y-full rather than snapping straight to its open position.
    this.panelTarget.classList.remove("invisible")
    void this.panelTarget.offsetHeight
    this.panelTarget.classList.remove("translate-y-full")
    this.panelTarget.classList.add("translate-y-0")
    this.backdropTarget.style.display = ""

    // Load the email content
    const frame = this.element.querySelector("turbo-frame")
    if (frame) frame.src = `/email_messages/${messageId}/drawer_content`
  }

  _keydown(event) {
    if (event.key === "Escape") {
      this.close()
    }
  }
}
