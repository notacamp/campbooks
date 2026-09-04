import { Controller } from "@hotwired/stimulus"

// Controls the Details rail / sheet beside a People conversation:
//   - xl+ (≥ 1280px): a static rail. open/close/toggle expand or collapse it (the
//     `hidden` attribute); the choice is remembered per browser in localStorage so
//     a collapsed rail stays collapsed from person to person.
//   - lg to < xl: a sheet (translate-x-full → translate-x-0) over the conversation,
//     closed by its Back/close control, Escape, or a click outside.
//   - < lg (phones): the sheet is full screen with a back button.
//
// Values:
//   personIdValue — used to set the lazy frame's src on first open when it has none.
//   openValue     — when true and below xl, the sheet opens on connect (?details=1).
//
// Actions: open · close · toggle · jump (scroll to a thread in the conversation).

const STORAGE_KEY = "people_details_rail_collapsed"

export default class extends Controller {
  static targets = ["pane", "detailsBtn"]
  static values  = { personId: String, open: Boolean }

  connect () {
    this._bound = {
      keydown: this._onKeydown.bind(this),
      click:   this._onOutsideClick.bind(this)
    }
    window.addEventListener("keydown", this._bound.keydown)
    if (this._atXl()) {
      // The rail: restore the remembered collapse. A hidden rail keeps its lazy
      // frame unloaded until it is expanded (Turbo loads lazy frames on visibility).
      if (this._storedCollapsed()) this._collapse({ remember: false })
    } else if (this.openValue) {
      // The sheet: open immediately when the page was loaded with ?details=1.
      this.open()
    }
  }

  disconnect () {
    window.removeEventListener("keydown", this._bound.keydown)
    document.removeEventListener("click", this._bound.click, true)
  }

  // ── Public actions ────────────────────────────────────────────────────────

  open () {
    if (!this.hasPaneTarget) return
    if (this._atXl()) { this._expand(); return }

    this.paneTarget.classList.remove("translate-x-full")
    this.paneTarget.classList.add("translate-x-0")
    this._ensureFrameSrc()
    // Trap click-outside to close — defer so the current click doesn't immediately close it.
    setTimeout(() => document.addEventListener("click", this._bound.click, true), 0)
    // Move focus into the sheet.
    const focusable = this.paneTarget.querySelector("button, [href], input, select")
    if (focusable) focusable.focus()
  }

  close () {
    if (!this.hasPaneTarget) return
    if (this._atXl()) { this._collapse({ remember: true }); return }

    this.paneTarget.classList.remove("translate-x-0")
    this.paneTarget.classList.add("translate-x-full")
    document.removeEventListener("click", this._bound.click, true)
    // Return focus to the details button if present.
    if (this.hasDetailsBtnTarget) this.detailsBtnTarget.focus()
  }

  toggle () {
    if (!this.hasPaneTarget) return
    if (this._atXl()) {
      this.paneTarget.hidden ? this.open() : this.close()
      return
    }
    this._isOpen() ? this.close() : this.open()
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
      // Close the sheet below xl; the rail stays where it is.
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

  _expand () {
    this.paneTarget.hidden = false
    this._ensureFrameSrc()
    this._store(false)
  }

  _collapse ({ remember }) {
    this.paneTarget.hidden = true
    if (!remember) return
    this._store(true)
    if (this.hasDetailsBtnTarget) this.detailsBtnTarget.focus()
  }

  // The frame ships with a lazy src; this only covers a frame rendered without one.
  _ensureFrameSrc () {
    const frame = this.paneTarget.querySelector("turbo-frame#people_details")
    if (frame && !frame.src && this.personIdValue) frame.src = `/people/${this.personIdValue}/details`
  }

  _storedCollapsed () {
    try { return window.localStorage.getItem(STORAGE_KEY) === "1" } catch { return false }
  }

  _store (collapsed) {
    try { window.localStorage.setItem(STORAGE_KEY, collapsed ? "1" : "0") } catch { /* private mode etc. */ }
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
