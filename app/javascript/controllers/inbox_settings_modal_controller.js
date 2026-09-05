import { Controller } from "@hotwired/stimulus"

// Applies localStorage-backed Display preferences (thread density, tag/attachment
// visibility, view mode) whenever the inbox re-renders. The open/close dialog
// surface is gone — settings now live in the Settings overlay. This controller
// stays on <body> in the email layout because the Display toggles apply globally.
export default class extends Controller {
  connect() {
    // Apply saved Display prefs on load, and re-apply whenever the inbox content
    // reloads — turbo:frame-load bubbles to document.
    this._onFrameLoad = () => this.restore()
    document.addEventListener("turbo:frame-load", this._onFrameLoad)
    this.restore()
  }

  disconnect() {
    document.removeEventListener("turbo:frame-load", this._onFrameLoad)
  }

  // --- Display preferences (localStorage) -----------------------------------

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
    document.querySelectorAll("[data-view-mode]").forEach((btn) => {
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
    document.querySelectorAll("[data-thread-view]").forEach((btn) => {
      const active = btn.dataset.threadView === current
      btn.classList.toggle("ring-2", active)
      btn.classList.toggle("ring-accent-500", active)
      btn.classList.toggle("bg-accent-50", active)
      btn.classList.toggle("bg-gray-100", !active)
      btn.classList.toggle("text-gray-600", !active)
    })
  }

  restore() {
    // Fallback reads the legacy "inbox_labels" key so existing users keep their preference.
    const tagsRaw = localStorage.getItem("inbox_tags") ?? localStorage.getItem("inbox_labels")
    this.apply("tags", tagsRaw !== "false")
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
    document.querySelectorAll("input[data-setting]").forEach((cb) => {
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
      case "tags":
        document.querySelectorAll(".js-inbox-tags").forEach((el) => { el.style.display = value ? "" : "none" })
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
}
