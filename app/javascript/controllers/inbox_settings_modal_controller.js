import { Controller } from "@hotwired/stimulus"

// Drives the inbox settings dialog (Campbooks::InboxSettingsModal).
//
// Responsibilities:
//  - open/close the native <dialog> (gear icon, Done button, backdrop, Esc)
//  - lazy-load the default panel into the content frame on first open
//  - highlight the active left-nav item (aria-current)
//  - restore/apply the localStorage-backed Display preferences whenever the
//    Display panel is (re)loaded into the frame
//  - honour ?inbox_settings=<section>[&id=<id>] deep-links on page load
export default class extends Controller {
  static targets = ["dialog", "panel", "navItem"]

  connect() {
    this.loaded = false
    // Apply saved Display prefs on load, and re-apply whenever the inbox content
    // (or any panel) reloads — turbo:frame-load bubbles to document. This takes
    // over what the old inbox-settings controller did from inside the frame.
    this._onFrameLoad = () => this.restore()
    document.addEventListener("turbo:frame-load", this._onFrameLoad)
    this.restore()
    this._maybeOpenFromDeepLink()
  }

  disconnect() {
    document.removeEventListener("turbo:frame-load", this._onFrameLoad)
  }

  // --- open / close -------------------------------------------------------

  open() {
    if (!this.hasDialogTarget) return
    if (!this.dialogTarget.open) this.dialogTarget.showModal()
    this._ensurePanelLoaded()
  }

  close() {
    if (this.hasDialogTarget && this.dialogTarget.open) this.dialogTarget.close()
  }

  // Native <dialog> renders its backdrop as the dialog element's own box, so a
  // click whose target IS the dialog (not its children) is a backdrop click.
  backdropClose(event) {
    if (event.target === this.dialogTarget) this.close()
  }

  _ensurePanelLoaded() {
    if (this.loaded || !this.hasPanelTarget) return
    this.loaded = true
    const src = this.panelTarget.getAttribute("src") || this.panelTarget.dataset.defaultSrc
    if (src) this.panelTarget.setAttribute("src", src)
  }

  // --- left-nav -----------------------------------------------------------

  setActive(event) {
    const item = event.currentTarget
    this._setActiveItem(item)
    // Let Turbo handle the frame navigation via the link's data-turbo-frame.
  }

  _setActiveItem(activeItem) {
    this.navItemTargets.forEach((el) => {
      if (el === activeItem) {
        el.setAttribute("aria-current", "page")
      } else {
        el.removeAttribute("aria-current")
      }
    })
  }

  // --- deep-link ----------------------------------------------------------

  _maybeOpenFromDeepLink() {
    const params = new URLSearchParams(window.location.search)
    const section = params.get("inbox_settings")
    if (!section) return

    const navItem = this.navItemTargets.find((el) => el.dataset.section === section)
    if (!navItem) return

    let src = navItem.getAttribute("href")
    const id = params.get("id")
    if (id) src += (src.includes("?") ? "&" : "?") + "id=" + encodeURIComponent(id)

    if (this.hasPanelTarget) {
      this.panelTarget.setAttribute("src", src)
      this.loaded = true
    }
    this._setActiveItem(navItem)
    this.open()
  }

  // --- Display preferences (localStorage; ported from inbox_settings) ------

  toggle(event) {
    const key = event.target.dataset.setting
    const value = event.target.checked
    localStorage.setItem(`inbox_${key}`, value)
    this.apply(key, value)
  }

  setViewMode(event) {
    const mode = event.currentTarget.dataset.viewMode
    localStorage.setItem("inbox_view_mode", mode)
    this.apply("view_mode", mode)
    this.syncViewModeButtons()
  }

  syncViewModeButtons() {
    const current = localStorage.getItem("inbox_view_mode") || "breathable"
    this._scope().querySelectorAll("[data-view-mode]").forEach((btn) => {
      if (btn.dataset.viewMode === current) {
        btn.classList.add("ring-2", "ring-accent-500", "bg-accent-50")
        btn.classList.remove("bg-gray-100", "text-gray-600")
      } else {
        btn.classList.remove("ring-2", "ring-accent-500", "bg-accent-50")
        btn.classList.add("bg-gray-100", "text-gray-600")
      }
    })
  }

  // Conversation view (how the reading pane renders a thread): bubbles | classic.
  setThreadView(event) {
    const view = event.currentTarget.dataset.threadView
    localStorage.setItem("inbox_thread_view", view)
    this.apply("thread_view", view)
    this.syncThreadViewButtons()
  }

  syncThreadViewButtons() {
    const current = localStorage.getItem("inbox_thread_view") || "bubbles"
    this._scope().querySelectorAll("[data-thread-view]").forEach((btn) => {
      const active = btn.dataset.threadView === current
      btn.classList.toggle("ring-2", active)
      btn.classList.toggle("ring-accent-500", active)
      btn.classList.toggle("bg-accent-50", active)
      btn.classList.toggle("bg-gray-100", !active)
      btn.classList.toggle("text-gray-600", !active)
    })
  }

  restore() {
    this.apply("labels", localStorage.getItem("inbox_labels") !== "false")
    this.apply("attachments", localStorage.getItem("inbox_attachments") !== "false")
    this.apply("chat", localStorage.getItem("inbox_chat") !== "false")
    this.apply("view_mode", localStorage.getItem("inbox_view_mode") || "breathable")
    this.apply("thread_view", localStorage.getItem("inbox_thread_view") || "bubbles")

    // Account visibility straight from localStorage, so hidden accounts stay
    // hidden across inbox re-renders even when the Display panel isn't open.
    for (let i = 0; i < localStorage.length; i++) {
      const key = localStorage.key(i)
      if (key && key.startsWith("inbox_account-")) {
        this.apply(key.replace("inbox_", ""), localStorage.getItem(key) !== "false")
      }
    }

    // Reflect saved state into the Display panel controls when they're rendered.
    this._scope().querySelectorAll("input[data-setting]").forEach((cb) => {
      cb.checked = localStorage.getItem(`inbox_${cb.dataset.setting}`) !== "false"
    })
    this.syncViewModeButtons()
    this.syncThreadViewButtons()
  }

  apply(key, value) {
    if (key && key.startsWith("account-")) {
      const accountId = key.replace("account-", "")
      document.querySelectorAll(`[data-email-account-id="${accountId}"]`).forEach((el) => {
        el.style.display = value ? "" : "none"
      })
      return
    }

    switch (key) {
      case "labels":
        document.querySelectorAll(".js-inbox-labels").forEach((el) => { el.style.display = value ? "" : "none" })
        break
      case "attachments":
        document.querySelectorAll(".js-inbox-attachments").forEach((el) => { el.style.display = value ? "" : "none" })
        break
      case "chat":
        document.querySelectorAll(".js-inbox-chat").forEach((el) => { el.style.display = value ? "" : "none" })
        break
      case "view_mode":
        document.querySelectorAll("#email_threads").forEach((el) => {
          if (value && value !== "default") {
            el.setAttribute("data-inbox-view-mode", value)
          } else {
            el.removeAttribute("data-inbox-view-mode")
          }
        })
        break
      case "thread_view":
        // Global flag (the reading pane + drawer both render bubbles); CSS flattens
        // them to the classic list. "bubbles" (default) = no attribute.
        if (value === "classic") {
          document.documentElement.setAttribute("data-thread-view", "classic")
        } else {
          document.documentElement.removeAttribute("data-thread-view")
        }
        break
    }
  }

  // Display controls live inside the dialog; scope queries there when possible.
  _scope() {
    return this.hasDialogTarget ? this.dialogTarget : this.element
  }
}
