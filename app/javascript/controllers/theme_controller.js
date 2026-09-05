import { Controller } from "@hotwired/stimulus"

// Manages the light/dark/system theme preference. Persists to localStorage.
// The initial class is applied by an inline anti-flash script in the layout
// <head> before first paint.
export default class extends Controller {
  static targets = ["sun", "moon", "option"]

  connect() {
    this.boundMediaChange = this._onMediaChange.bind(this)
    this._media = window.matchMedia("(prefers-color-scheme: dark)")
    this._media.addEventListener("change", this.boundMediaChange)
    this.render()
  }

  disconnect() {
    this._media.removeEventListener("change", this.boundMediaChange)
  }

  // Legacy toggle (kept for any remaining callers: theme#toggle)
  toggle() {
    const isDark = document.documentElement.classList.toggle("dark")
    try { localStorage.setItem("theme", isDark ? "dark" : "light") } catch (e) {}
    this.render()
  }

  // New three-way set from UserMenu segmented control
  set(event) {
    const mode = event.params.mode
    if (mode === "dark") {
      document.documentElement.classList.add("dark")
      try { localStorage.setItem("theme", "dark") } catch (e) {}
    } else if (mode === "light") {
      document.documentElement.classList.remove("dark")
      try { localStorage.setItem("theme", "light") } catch (e) {}
    } else {
      // system
      try { localStorage.removeItem("theme") } catch (e) {}
      if (this._media.matches) {
        document.documentElement.classList.add("dark")
      } else {
        document.documentElement.classList.remove("dark")
      }
    }
    this.render()
  }

  render() {
    const isDark = document.documentElement.classList.contains("dark")
    if (this.hasSunTarget) this.sunTarget.classList.toggle("hidden", !isDark)
    if (this.hasMoonTarget) this.moonTarget.classList.toggle("hidden", isDark)

    // Sync aria-pressed on segmented option buttons
    if (this.hasOptionTarget) {
      let stored
      try { stored = localStorage.getItem("theme") } catch (e) { stored = null }
      const active = stored || "system"
      this.optionTargets.forEach((btn) => {
        const mode = btn.dataset.themeModeParam
        btn.setAttribute("aria-pressed", String(mode === active))
      })
    }
  }

  _onMediaChange() {
    let stored
    try { stored = localStorage.getItem("theme") } catch (e) { stored = null }
    if (!stored) {
      // system mode: follow the OS
      if (this._media.matches) {
        document.documentElement.classList.add("dark")
      } else {
        document.documentElement.classList.remove("dark")
      }
      this.render()
    }
  }
}
