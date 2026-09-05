import { Controller } from "@hotwired/stimulus"

// Drives the global settings overlay (<dialog id="settings-overlay">).
// Lives on <body> like scout-overlay — both operate independently.
//
// State model: the overlay is open iff location.pathname starts with /settings.
// Everything derives from that single rule.
const SETTINGS_RE = /^\/settings(\/|$)/

export default class extends Controller {
  static targets = ["dialog", "frame", "nav", "pane", "navItem", "group", "find"]
  static values  = {
    defaultUrl: { type: String, default: "/settings/account" },
    returnUrl:  { type: String, default: "/now" }
  }

  connect() {
    this._returnTo = null
    this._closing  = false

    this._boundKeydown   = this._handleKeydown.bind(this)
    this._boundTurboLoad = () => this.syncToUrl()

    document.addEventListener("keydown", this._boundKeydown)
    document.addEventListener("turbo:load", this._boundTurboLoad)

    if (this.hasFrameTarget) {
      this._boundFrameLoad    = () => this._afterFrameLoad()
      this._boundFrameMissing = (e) => {
        e.preventDefault()
        e.detail.visit(e.detail.response)
      }
      this.frameTarget.addEventListener("turbo:frame-load",    this._boundFrameLoad)
      this.frameTarget.addEventListener("turbo:frame-missing", this._boundFrameMissing)
    }

    this.syncToUrl()
  }

  disconnect() {
    document.removeEventListener("keydown", this._boundKeydown)
    document.removeEventListener("turbo:load", this._boundTurboLoad)
    if (this.hasFrameTarget) {
      this.frameTarget.removeEventListener("turbo:frame-load",    this._boundFrameLoad)
      this.frameTarget.removeEventListener("turbo:frame-missing", this._boundFrameMissing)
    }
  }

  // ── URL sync ──────────────────────────────────────────────────────────────

  syncToUrl() {
    if (!this.hasDialogTarget) return
    const dialog = this.dialogTarget

    if (SETTINGS_RE.test(location.pathname)) {
      // On a settings URL — ensure the overlay is open and the frame is in sync.
      if (dialog.open && !dialog.matches(":modal")) {
        // Restored Turbo snapshot: dialog is open but not modal — reset it.
        this._closing = true
        dialog.close()
        this._closing = false
      }
      if (!dialog.open) {
        try { dialog.showModal() } catch (e) {}
      }
      // Sync the frame if the server-rendered current-url differs.
      if (this.hasFrameTarget) {
        const frameUrl  = this.frameTarget.dataset.currentUrl
        const pageUrl   = location.href.split("#")[0]
        if (!frameUrl || frameUrl.split("#")[0] !== pageUrl) {
          this.frameTarget.src = location.href
        }
      }
      this._setMode("detail")
    } else {
      // Not a settings URL — close silently if open.
      if (dialog.open) {
        this._closing = true
        dialog.close()
        this._closing = false
      }
    }
  }

  // ── Open / close ──────────────────────────────────────────────────────────

  open(event) {
    if (event) event.preventDefault()
    if (!this.hasDialogTarget) return

    const url = event?.params?.url
      || event?.currentTarget?.href
      || this.defaultUrlValue

    if (!SETTINGS_RE.test(location.pathname)) {
      this._returnTo = location.href
    }

    if (!this.dialogTarget.open) {
      try { this.dialogTarget.showModal() } catch (e) {}
    }
    this._setMode("detail")

    if (this.hasFrameTarget && url) {
      this.frameTarget.src = url
    }
  }

  navigate(event) {
    // Highlight the clicked item immediately (Turbo handles frame load).
    this._highlightNavItem(event.currentTarget)
    this._setMode("detail")
    // Scroll the pane to top on phone.
    if (this.hasPaneTarget) this.paneTarget.scrollTop = 0
  }

  back() {
    this._setMode("index")
  }

  close() {
    if (this.hasDialogTarget && this.dialogTarget.open) {
      this.dialogTarget.close()
    }
  }

  onClose() {
    if (this._closing) return
    if (SETTINGS_RE.test(location.pathname)) {
      const returnTo = this._returnTo || this.returnUrlValue
      this._returnTo = null
      if (typeof Turbo !== "undefined") {
        Turbo.visit(returnTo, { action: "replace" })
      } else {
        location.href = returnTo
      }
    }
    this._returnTo = null
  }

  backdropClose(event) {
    if (event.target === this.dialogTarget) this.close()
  }

  // ── Frame hooks ───────────────────────────────────────────────────────────

  _afterFrameLoad() {
    if (!this.hasFrameTarget) return
    this.frameTarget.dataset.currentUrl = this.frameTarget.src || location.href

    // Highlight the nav item whose href pathname matches the loaded URL.
    const loadedPath = new URL(this.frameTarget.src || location.href, location.origin).pathname
    const matched = this.navItemTargets.find((el) => {
      return new URL(el.href, location.origin).pathname === loadedPath
    })
    if (matched) this._highlightNavItem(matched)

    // Move focus to the first heading in the pane (accessibility).
    if (this.hasPaneTarget) {
      const h = this.paneTarget.querySelector("h1, h2")
      if (h) h.focus({ preventScroll: true })
    }
  }

  // ── Filter ────────────────────────────────────────────────────────────────

  filter(event) {
    const q = (event.target.value || "").toLowerCase().trim()

    this.navItemTargets.forEach((item) => {
      const text  = item.textContent.toLowerCase()
      const match = !q || text.includes(q)
      item.classList.toggle("hidden", !match)
    })

    // Hide a group when all its items are hidden.
    this.groupTargets.forEach((group) => {
      const hasVisible = group.querySelectorAll("[data-settings-overlay-target='navItem']:not(.hidden)").length > 0
      group.classList.toggle("hidden", !hasVisible)
    })
  }

  // ── Private helpers ───────────────────────────────────────────────────────

  _handleKeydown(e) {
    // ⌘, or Ctrl+, → open settings.
    if ((e.metaKey || e.ctrlKey) && e.key === ",") {
      e.preventDefault()
      this.open(e)
      return
    }
    // / → focus find box when overlay is open and no editable is focused.
    if (e.key === "/" && this.hasDialogTarget && this.dialogTarget.open && !this._isEditable(e.target)) {
      e.preventDefault()
      if (this.hasFindTarget) this.findTarget.focus()
    }
  }

  _isEditable(el) {
    if (!el) return false
    const tag = el.tagName
    return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || el.isContentEditable
  }

  _setMode(mode) {
    if (this.hasDialogTarget) this.dialogTarget.dataset.mode = mode
  }

  _highlightNavItem(active) {
    this.navItemTargets.forEach((el) => {
      if (el === active) {
        el.setAttribute("aria-current", "page")
      } else {
        el.removeAttribute("aria-current")
      }
    })
  }
}
