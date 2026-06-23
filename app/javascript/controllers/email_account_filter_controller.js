import { Controller } from "@hotwired/stimulus"

// Client-side per-account filter for the inbox thread list. Clicking an avatar
// toggles its account's rows in/out. The selection is persisted in sessionStorage
// so it survives opening an email — clicking a row is a full Turbo navigation that
// rebuilds the list, which used to silently drop the filter — and stays put for the
// rest of the browser session. Clicking the avatar again clears it; a new tab/
// session starts unfiltered. The list also grows via Turbo Stream appends
// (infinite scroll), so we watch #email_threads and re-apply the filter whenever
// new rows land — otherwise emails from a deselected account reappear on later pages.
const STORAGE_KEY = "campbooks:inbox:hidden-accounts"

export default class extends Controller {
  static targets = ["toggle"]

  connect() {
    this.hiddenAccountIds = this.#restore()
    this.#observeThreads()
    // Re-apply a restored selection to the freshly-rendered list + avatars.
    if (this.hiddenAccountIds.size > 0) this.updateUI()
  }

  disconnect() {
    this.observer?.disconnect()
  }

  toggle(event) {
    const accountId = event.currentTarget.dataset.emailAccountId
    if (this.hiddenAccountIds.has(accountId)) {
      this.hiddenAccountIds.delete(accountId)
    } else {
      this.hiddenAccountIds.add(accountId)
    }
    this.#persist()
    this.updateUI()
  }

  updateUI() {
    this.toggleTargets.forEach((toggle) => {
      toggle.classList.toggle("opacity-25", this.hiddenAccountIds.has(toggle.dataset.emailAccountId))
    })
    this.#applyToRows()
  }

  #applyToRows() {
    const threads = document.getElementById("email_threads")
    if (!threads) return
    threads.querySelectorAll("[data-email-account-id]").forEach((item) => {
      item.classList.toggle("hidden", this.hiddenAccountIds.has(item.dataset.emailAccountId))
    })
  }

  // Re-apply the filter to rows appended by infinite scroll. Guarded on a
  // non-empty selection so an unfiltered list never pays for the scan.
  #observeThreads() {
    const threads = document.getElementById("email_threads")
    if (!threads) return
    this.observer = new MutationObserver(() => {
      if (this.hiddenAccountIds.size > 0) this.#applyToRows()
    })
    this.observer.observe(threads, { childList: true })
  }

  #restore() {
    try {
      const raw = sessionStorage.getItem(STORAGE_KEY)
      return new Set(raw ? JSON.parse(raw) : [])
    } catch {
      // sessionStorage unavailable (private mode / blocked) — fall back to in-memory.
      return new Set()
    }
  }

  #persist() {
    try {
      sessionStorage.setItem(STORAGE_KEY, JSON.stringify([...this.hiddenAccountIds]))
    } catch {
      // Storage write failed (quota/blocked) — the in-memory Set still drives this view.
    }
  }
}
