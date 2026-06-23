import { Controller } from "@hotwired/stimulus"

// Toggles the `.dark` class on <html>, persists the choice to localStorage,
// and keeps the topbar toggle icon in sync. The initial class is applied by
// an inline anti-flash script in the layout <head> before first paint.
export default class extends Controller {
  static targets = ["sun", "moon"]

  connect() {
    this.render()
  }

  toggle() {
    const isDark = document.documentElement.classList.toggle("dark")
    try {
      localStorage.setItem("theme", isDark ? "dark" : "light")
    } catch (e) {}
    this.render()
  }

  render() {
    const isDark = document.documentElement.classList.contains("dark")
    // Sun = "switch to light" (shown while dark); Moon = "switch to dark".
    if (this.hasSunTarget) this.sunTarget.classList.toggle("hidden", !isDark)
    if (this.hasMoonTarget) this.moonTarget.classList.toggle("hidden", isDark)
  }
}
