import { Controller } from "@hotwired/stimulus"

// Debounced live match count for the rule builder form.
// Watches all [data-rule-match-count-target="field"] inputs, debounces 400ms,
// fetches the match_count endpoint, and updates count targets.
export default class extends Controller {
  static targets = ["field", "count", "countInline", "display"]
  static values  = { url: String }

  connect() {
    this._timer = null
    this._fetch()
  }

  disconnect() {
    clearTimeout(this._timer)
  }

  debounce() {
    clearTimeout(this._timer)
    this._timer = setTimeout(() => this._fetch(), 400)
  }

  async _fetch() {
    const params = new URLSearchParams()

    this.fieldTargets.forEach((el) => {
      if (!el.name) return
      // Translate email_rule[criteria][foo] -> criteria[foo]
      const key = el.name.replace(/^email_rule\[criteria\]/, "criteria")
      if (el.type === "checkbox") {
        if (el.checked) params.append(key, el.value)
      } else if (el.value.trim()) {
        params.append(key, el.value)
      }
    })

    try {
      const resp = await fetch(`${this.urlValue}?${params.toString()}`, {
        headers: { Accept: "application/json", "X-Requested-With": "XMLHttpRequest" }
      })
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`)
      const { count } = await resp.json()
      const formatted = count.toLocaleString()
      this.countTargets.forEach((el) => { el.textContent = formatted })
      this.countInlineTargets.forEach((el) => { el.textContent = formatted })
      this.displayTargets.forEach((el) => { el.hidden = false })
    } catch (_) {
      this.displayTargets.forEach((el) => { el.hidden = true })
    }
  }
}
