import { Controller } from "@hotwired/stimulus"

// Manages the Dock lifecycle around the streamed-in sheet: entrance animation
// with the app-shell stepping back, Esc/scrim minimize (never destroy), the
// same-page minimized pill, mode switching (reply ↔ reply-all ↔ forward, body
// carried), and expanding a draft into the Desk. Lives on #compose_dock_root
// in the layout so it survives every Turbo Stream swap of the slot.
export default class extends Controller {
  static targets = ["scrim", "sheet", "pill", "dockPill", "dockPillTitle"]

  disconnect() {
    this._scaleBack(false)
  }

  sheetTargetConnected(sheet) {
    this._minimized = false
    this._hide(this.hasPillTarget ? this.pillTarget : null)
    this._scaleBack(true)
    this._trap = this._trapTab.bind(this)
    sheet.addEventListener("keydown", this._trap)
    // Double rAF so the initial translate-y-full paints before transitioning.
    requestAnimationFrame(() => requestAnimationFrame(() => {
      sheet.classList.remove("translate-y-full")
      if (this.hasScrimTarget) this.scrimTarget.classList.add("opacity-100")
    }))
  }

  sheetTargetDisconnected(sheet) {
    sheet.removeEventListener("keydown", this._trap)
    this._scaleBack(false)
    this._show(this.hasPillTarget ? this.pillTarget : null)
  }

  // ── minimize / restore (same-page; DOM and editor state kept) ──
  minimize(event) {
    if (!this.hasSheetTarget || this._minimized) return
    if (event?.type === "keydown") {
      // Esc inside an open dropdown/popover should close that first.
      const openDetails = this.sheetTarget.querySelector("details[open]")
      if (openDetails) { openDetails.removeAttribute("open"); return }
    }

    const draftId = this._form()?.querySelector('input[name="draft_email_id"]')?.value
    if (!draftId && !this._hasContent()) return this.clear()

    this._minimized = true
    this.sheetTarget.classList.add("translate-y-full")
    // The sheet's upward shadow would still bleed into the viewport from
    // below the fold — silence it while parked.
    this.sheetTarget.style.boxShadow = "none"
    if (this.hasScrimTarget) {
      this.scrimTarget.classList.remove("opacity-100")
      this.scrimTarget.classList.add("pointer-events-none")
    }
    this._scaleBack(false)
    if (this.hasDockPillTarget) {
      if (this.hasDockPillTitleTarget) this.dockPillTitleTarget.textContent = this._pillTitle()
      this._show(this.dockPillTarget)
    }
  }

  restore(event) {
    event?.preventDefault()
    if (!this.hasSheetTarget) return
    this._minimized = false
    this._hide(this.hasDockPillTarget ? this.dockPillTarget : null)
    this.sheetTarget.style.boxShadow = ""
    this.sheetTarget.classList.remove("translate-y-full")
    if (this.hasScrimTarget) {
      this.scrimTarget.classList.add("opacity-100")
      this.scrimTarget.classList.remove("pointer-events-none")
    }
    this._scaleBack(true)
  }

  // Discard (from the engine) or an empty minimize: drop the sheet entirely.
  clear() {
    const slot = this.element.querySelector("#compose_dock")
    if (slot) slot.innerHTML = ""
  }

  // ── mode switching ───────────────────────────────────────────
  switchMode(event) {
    const mode = event.params.mode
    const messageId = this.hasSheetTarget ? this.sheetTarget.dataset.messageId : null
    if (!messageId || !mode) return
    this.sheetTarget.querySelector("details[open]")?.removeAttribute("open")

    const form = this._form()
    const params = new URLSearchParams({
      mode,
      body: form?.querySelector('input[name="body"]')?.value || "",
      draft_email_id: form?.querySelector('input[name="draft_email_id"]')?.value || ""
    })
    fetch(`/email_messages/${messageId}/compose`, {
      method: "POST",
      headers: {
        "X-CSRF-Token": this._csrf(),
        "Accept": "text/vnd.turbo-stream.html",
        "Content-Type": "application/x-www-form-urlencoded"
      },
      body: params.toString()
    }).then((r) => r.text()).then((html) => {
      if (html) Turbo.renderStreamMessage(html)
    })
  }

  // ── expand to the Desk ───────────────────────────────────────
  async expandToDesk() {
    const autosave = this._autosaveController()
    const draftId = autosave ? await autosave.ensureDraft() : ""
    let url = "/email_messages/new"
    if (draftId) {
      url += `?draft_id=${encodeURIComponent(draftId)}`
    } else if (this.hasSheetTarget && this.sheetTarget.dataset.messageId) {
      url += `?mode=${encodeURIComponent(this.sheetTarget.dataset.mode)}&reply_to=${encodeURIComponent(this.sheetTarget.dataset.messageId)}`
    }
    Turbo.visit(url)
  }

  // ── internals ────────────────────────────────────────────────
  _form() {
    return this.hasSheetTarget ? this.sheetTarget.querySelector("form") : null
  }

  _hasContent() {
    const form = this._form()
    if (!form) return false
    const body = form.querySelector('input[name="body"]')?.value || ""
    const stripped = body.replace(/<[^>]+>/g, "").trim()
    const subject = form.querySelector('input[name="subject"]')?.value?.trim()
    const to = form.querySelector('input[name="to_address"]')?.value?.trim()
    return Boolean(stripped || subject || to)
  }

  _pillTitle() {
    const form = this._form()
    const subject = form?.querySelector('input[name="subject"]')?.value?.trim()
    if (subject) return subject
    const to = form?.querySelector('input[name="to_address"]')?.value || ""
    return to.split(",")[0]?.trim() || ""
  }

  _scaleBack(on) {
    const shell = document.getElementById("app-shell")
    if (!shell) return
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return
    shell.classList.add("transition-transform", "duration-300", "origin-top")
    shell.classList.toggle("scale-[0.99]", on)
  }

  _trapTab(event) {
    if (event.key !== "Tab" || !this.hasSheetTarget || this._minimized) return
    const focusables = this.sheetTarget.querySelectorAll(
      "a[href], button:not([disabled]), input:not([type='hidden']):not([disabled]), select, textarea, [contenteditable='true'], summary"
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

  _autosaveController() {
    const form = this._form()
    return form && this.application.getControllerForElementAndIdentifier(form, "compose-autosave")
  }

  _csrf() {
    return document.querySelector("meta[name='csrf-token']")?.content || ""
  }

  _show(el) { el?.classList.remove("hidden"); el?.classList.add("flex") }
  _hide(el) { el?.classList.add("hidden"); el?.classList.remove("flex") }
}
