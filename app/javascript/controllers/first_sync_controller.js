import { Controller } from "@hotwired/stimulus"

// Drives the first-sync stage (Campbooks::FirstSyncStage): polls the status
// endpoint while Scout's first scan runs, tweens the counters up as mail lands,
// and plays the finish beat (halo → check, headline flip, CTA) when the scan
// completes. All copy comes from data-tmpl-* attributes rendered server-side —
// nothing here is user-facing text.
const POLL_MS = 1600
const TWEEN_MS = 600

export default class extends Controller {
  static targets = ["halo", "check", "title", "subtitle", "found", "sorted", "needsYou", "doneCta", "errorNote", "escape"]
  static values = {
    url: String,
    state: { type: String, default: "waiting" },
    feedPath: { type: String, default: "/" },
    revealDelay: { type: Number, default: 12000 }
  }

  connect() {
    this.counts = { found: this.int(this.foundTarget), sorted: this.int(this.sortedTarget), needs_you: this.int(this.needsYouTarget) }
    this.reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches
    this.finished = false
    this.apply({ state: this.stateValue, ...this.counts }, { initial: true })
    this.timer = setInterval(() => this.poll(), POLL_MS)
    // Slow scans shouldn't trap anyone — surface the escape hatch after a while.
    this.escapeTimer = setTimeout(() => this.showEscape(), this.revealDelayValue)
  }

  disconnect() {
    clearInterval(this.timer)
    clearTimeout(this.escapeTimer)
  }

  async poll() {
    if (this.finished) return
    try {
      const response = await fetch(this.urlValue, { headers: { Accept: "application/json" } })
      if (!response.ok) return
      this.apply(await response.json())
    } catch {
      // Transient network blip — keep polling.
    }
  }

  apply(status, { initial = false } = {}) {
    this.tween("found", this.foundTarget, status.found)
    this.tween("sorted", this.sortedTarget, status.sorted)
    this.tween("needs_you", this.needsYouTarget, status.needs_you)

    switch (status.state) {
      case "waiting":
        this.setText(this.subtitleTarget, this.tmpl(this.subtitleTarget, "waiting"))
        break
      case "scanning":
        this.setText(this.titleTarget, this.tmpl(this.titleTarget, "scanning"))
        this.setText(this.subtitleTarget, this.tmpl(this.subtitleTarget, "scanning"))
        break
      case "done":
        this.finish("done", status)
        break
      case "empty":
        this.finish("empty", status)
        break
      case "error":
        this.showError(initial)
        break
    }
  }

  // The payoff beat: stop the halo, pop the check, flip the copy, reveal the CTA.
  finish(kind, status) {
    if (this.finished) return
    this.finished = true
    clearInterval(this.timer)
    clearTimeout(this.escapeTimer)

    if (this.hasHaloTarget) this.haloTarget.classList.add("hidden")
    if (this.hasCheckTarget) {
      this.checkTarget.classList.remove("hidden")
      this.checkTarget.classList.add("flex", "animate-sync-done-pop")
    }
    this.setText(this.titleTarget, this.tmpl(this.titleTarget, kind))
    const subtitle = kind === "empty"
      ? this.tmpl(this.subtitleTarget, "empty")
      : (status.needs_you > 0
          ? this.tmpl(this.subtitleTarget, "done").replace("{needs_you}", status.needs_you)
          : this.tmpl(this.subtitleTarget, "doneCalm"))
    this.setText(this.subtitleTarget, subtitle)
    if (this.hasEscapeTarget) this.escapeTarget.classList.add("hidden")
    if (this.hasDoneCtaTarget) {
      this.doneCtaTarget.classList.remove("hidden")
      this.doneCtaTarget.classList.add("flex", "animate-stage-in")
    }
    // Hide the persona card (picker or confirmation) so it doesn't hang below the done CTA.
    const persona = document.getElementById("first-sync-persona")
    if (persona) persona.classList.add("hidden")
  }

  showError(initial) {
    this.setText(this.titleTarget, this.tmpl(this.titleTarget, "error"))
    this.setText(this.subtitleTarget, this.tmpl(this.subtitleTarget, "error"))
    if (this.hasHaloTarget) this.haloTarget.classList.add("hidden")
    if (this.hasErrorNoteTarget) {
      this.errorNoteTarget.classList.remove("hidden")
      this.errorNoteTarget.classList.add("flex")
      if (!initial) this.errorNoteTarget.classList.add("animate-stage-in")
    }
    // Keep polling — the next scheduled scan may recover on its own.
  }

  showEscape() {
    if (this.finished || !this.hasEscapeTarget) return
    this.escapeTarget.classList.remove("hidden")
    this.escapeTarget.classList.add("animate-stage-in")
  }

  reveal() {
    window.Turbo ? window.Turbo.visit(this.feedPathValue) : (window.location.href = this.feedPathValue)
  }

  // ── counters ────────────────────────────────────────────────────────────────

  tween(key, el, to) {
    to = Number(to) || 0
    const from = this.counts[key] ?? 0
    if (to === from) return
    this.counts[key] = to
    if (this.reduced) { el.textContent = to; return }

    const start = performance.now()
    const step = (now) => {
      const p = Math.min((now - start) / TWEEN_MS, 1)
      const eased = 1 - Math.pow(1 - p, 3)
      el.textContent = Math.round(from + (to - from) * eased)
      if (p < 1) requestAnimationFrame(step)
    }
    requestAnimationFrame(step)
  }

  // ── helpers ────────────────────────────────────────────────────────────────

  int(el) { return parseInt(el?.textContent, 10) || 0 }
  tmpl(el, key) { return el?.dataset[`tmpl${key.charAt(0).toUpperCase()}${key.slice(1)}`] || "" }
  setText(el, text) { if (el && text && el.textContent !== text) el.textContent = text }
}
