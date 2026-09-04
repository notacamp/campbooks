import { Controller } from "@hotwired/stimulus"

// Controls the Details rail / sheet:
//   - xl+ (≥ 1280px): rail is always visible (static aside). Controller is a no-op for open/close.
//   - lg to < xl: aside is a sheet (translate-x-full → translate-x-0) over the conversation.
//   - < lg (phones): sheet is w-full (full screen) with a back button.
//
// Values:
//   personIdValue — used to match the turbo-frame src when lazy-loading.
//   openValue     — when true and below xl, the sheet opens immediately on connect
//                   (set from params[:details] in the conversation partial).
//
// Actions:
//   open    — slide in the sheet (< xl only)
//   close   — slide out the sheet
//   toggle  — toggle open/close
//   jump    — scroll to a thread in the conversation pane

export default class extends Controller {
  static targets = ["pane", "detailsBtn"]
  static values  = { personId: String, open: Boolean }

  connect () {
    this._bound = {
      keydown: this._onKeydown.bind(this),
      click:   this._onOutsideClick.bind(this)
    }
    window.addEventListener("keydown", this._bound.keydown)
    // Open the sheet immediately when the page was loaded with ?details=1.
    if (this.openValue && !this._atXl()) {
      this.open()
    }
  }

  disconnect () {
    window.removeEventListener("keydown", this._bound.keydown)
    document.removeEventListener("click", this._bound.click, true)
  }

  // ── Public actions ────────────────────────────────────────────────────────

  open () {
    if (this._atXl()) return
    if (!this.hasPaneTarget) return
    this.paneTarget.classList.remove("translate-x-full")
    this.paneTarget.classList.add("translate-x-0")
    // Lazy-load: set src on the frame if it hasn't been loaded yet.
    const frame = this.paneTarget.querySelector("turbo-frame[id='people_details']")
    if (frame && !frame.src) {
      const personId = this.personIdValue
      if (personId) frame.src = `/people/${personId}/details`
    }
    // Trap click-outside to close — defer so the current click doesn't immediately close it.
    setTimeout(() => document.addEventListener("click", this._bound.click, true), 0)
    // Move focus into the sheet.
    const focusable = this.paneTarget.querySelector("button, [href], input, select")
    if (focusable) focusable.focus()
  }

  close () {
    if (!this.hasPaneTarget) return
    this.paneTarget.classList.remove("translate-x-0")
    this.paneTarget.classList.add("translate-x-full")
    document.removeEventListener("click", this._bound.click, true)
    // Return focus to the details button if present.
    if (this.hasDetailsBtnTarget) this.detailsBtnTarget.focus()
  }

  toggle () {
    if (this._isOpen()) {
      this.close()
    } else {
      this.open()
    }
  }

  // Scrolls to a thread block in the conversation pane.
  // Called from the Threads list in the Details component (data-action="click->people-details#jump").
  jump (event) {
    const threadId = event.params.threadId
    if (!threadId) return

    // Look for the thread block by id.
    const block = document.getElementById(`people_thread_block_${threadId}`) ||
                  document.getElementById(`people_thread_${threadId}`)?.parentElement

    if (block) {
      event.preventDefault()
      block.scrollIntoView({ block: "start", behavior: "smooth" })
      // Ring highlight for 1.5 s.
      block.classList.add("ring-2", "ring-primary/40", "ring-offset-1", "rounded-xl", "transition-all")
      setTimeout(() => block.classList.remove("ring-2", "ring-primary/40", "ring-offset-1"), 1500)
      // Close the sheet below xl.
      if (!this._atXl()) this.close()
    }
    // If not in DOM, follow the link normally (navigate to person page with ?thread=).
  }

  // ── Private ───────────────────────────────────────────────────────────────

  _atXl () {
    return window.matchMedia("(min-width: 1280px)").matches
  }

  _isOpen () {
    return this.hasPaneTarget && this.paneTarget.classList.contains("translate-x-0")
  }

  _onKeydown (event) {
    if (event.key === "Escape" && this._isOpen() && !this._atXl()) {
      this.close()
    }
  }

  _onOutsideClick (event) {
    if (!this.hasPaneTarget) return
    // Ignore clicks on opener buttons — otherwise the capture-phase close fires
    // first and the toggle immediately reopens, making the button unable to close.
    if (event.target.closest("[data-action*='people-details#toggle'], [data-action*='people-details#open']")) return
    if (!this.paneTarget.contains(event.target)) {
      this.close()
    }
  }
}
