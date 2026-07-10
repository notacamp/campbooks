import { Controller } from "@hotwired/stimulus"

// Drives the walkthrough v2 (Campbooks::ProductTour). An explanation-first,
// stories-style slideshow: six slides about what Campbooks is and what each
// module does. Tap-through only — no auto-advance in-app. Segmented progress
// bars (click to jump), keyboard ← → / Escape, and per-slide vignette
// animations (chips pop, counter ticks, buttons morph to a ✓ state).
//
// Mounted on <body> so any "Take the tour" button (data-action="product-tour#open")
// and the overlay's own controls share one controller. Nothing touches the
// user's real workspace; the only request is the one-time "seen" POST on close.
const DISMISS_URL = "/tours/product_tour/dismiss"

const REDUCED = typeof matchMedia !== "undefined" && matchMedia("(prefers-reduced-motion: reduce)").matches

// Stroke icons for the deck-head module label, keyed by slide type.
const ICO = {
  intro: '<svg viewBox="0 0 24 24" fill="currentColor" class="h-3.5 w-3.5" aria-hidden="true"><path d="M12 2l1.7 5.6L19.5 9l-5.8 1.4L12 16l-1.7-5.6L4.5 9l5.8-1.4z"/></svg>',
  inbox: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="h-3.5 w-3.5" aria-hidden="true"><path d="M22 12h-6l-2 3h-4l-2-3H2"/><path d="M5.45 5.11 2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z"/></svg>',
  calendar: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="h-3.5 w-3.5" aria-hidden="true"><rect x="3" y="4.5" width="18" height="16.5" rx="2"/><path d="M3 9.5h18M8 3v4M16 3v4"/></svg>',
  tasks: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="h-3.5 w-3.5" aria-hidden="true"><path d="M9 11l3 3L22 4"/><path d="M21 12v7a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11"/></svg>',
  docs: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="h-3.5 w-3.5" aria-hidden="true"><path d="M14.5 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7.5z"/><path d="M14 2v6h6M16 13H8M16 17H8M10 9H8"/></svg>',
  more: '<svg viewBox="0 0 24 24" fill="currentColor" class="h-3.5 w-3.5" aria-hidden="true"><path d="M12 2l1.7 5.6L19.5 9l-5.8 1.4L12 16l-1.7-5.6L4.5 9l5.8-1.4z"/></svg>'
}

export default class extends Controller {
  static targets = ["panel", "slide", "segment", "segFill", "modLabel", "countLabel", "prevBtn", "nextBtn"]

  connect() {
    this.idx = 0
    this.opened = false
    this._timers = []
    this._rotTimer = null

    const wantsTour = new URLSearchParams(window.location.search).has("tour")
    const autostart = this.hasPanelTarget && this.panelTarget.dataset.tourAutostart === "true"
    if (wantsTour || autostart) requestAnimationFrame(() => this.open())
  }

  disconnect() {
    this._clearAll()
    if (this.opened) document.documentElement.style.overflow = ""
  }

  // ── open / close ──────────────────────────────────────────────────────────

  open(event) {
    if (event) event.preventDefault()
    if (!this.hasPanelTarget) return
    this.opened = true
    this.idx = 0
    this._clearAll()
    this._resetSlides()
    this.panelTarget.classList.remove("hidden")
    this.panelTarget.classList.add("flex")
    document.documentElement.style.overflow = "hidden"
    this._go(0)
    this.panelTarget.focus({ preventScroll: true })
  }

  close() {
    if (!this.hasPanelTarget) return
    this._clearAll()
    this.panelTarget.classList.add("hidden")
    this.panelTarget.classList.remove("flex")
    document.documentElement.style.overflow = ""
    this.opened = false
  }

  skip(event) {
    if (event) event.preventDefault()
    this._markSeen()
    this.close()
  }

  // "Connect your inbox" / "Back to your inbox" CTA on the last slide.
  finishConnect(event) {
    if (event) event.preventDefault()
    const path = event?.currentTarget?.dataset.tourConnectPath
    if (!path) { this.skip(); return }
    this._markSeen()
    window.location.href = path
  }

  // ── navigation ────────────────────────────────────────────────────────────

  next(event) {
    if (event) event.preventDefault()
    if (this.idx >= this.slideTargets.length - 1) return
    this._go(this.idx + 1)
  }

  prev(event) {
    if (event) event.preventDefault()
    if (this.idx <= 0) return
    this._go(this.idx - 1)
  }

  // Click on a progress-bar segment to jump to that slide.
  goTo(event) {
    if (event) event.preventDefault()
    const n = parseInt(event.currentTarget.dataset.tourSegmentIndex, 10)
    if (!isNaN(n)) this._go(n)
  }

  onKeydown(event) {
    if (!this.opened) return
    switch (event.key) {
      case "Escape":     this.skip(); break
      case "ArrowRight": this.next(); break
      case "ArrowLeft":  this.prev(); break
      default: return
    }
    event.preventDefault()
  }

  // ── internal render ────────────────────────────────────────────────────────

  _go(n) {
    this._clearAll()
    const total = this.slideTargets.length
    n = Math.max(0, Math.min(n, total - 1))
    const prev = this.idx
    this.idx = n
    const slideEl = this.slideTargets[n]

    // Progress bar segments
    this.segFillTargets.forEach((fill, i) => {
      fill.style.transition = "none"
      fill.style.transform = i < n ? "scaleX(1)" : "scaleX(0)"
    })
    // Animate the current segment fill in (0.4s)
    requestAnimationFrame(() => requestAnimationFrame(() => {
      if (this.segFillTargets[n]) {
        this.segFillTargets[n].style.transition = "transform 0.4s ease"
        this.segFillTargets[n].style.transform = "scaleX(1)"
      }
    }))

    // Module label
    if (this.hasModLabelTarget) {
      const type = slideEl.dataset.tourSlideType || ""
      const label = slideEl.dataset.tourModLabel || ""
      this.modLabelTarget.innerHTML = `${ICO[type] || ""}${label ? `<span>${label}</span>` : ""}`
    }

    // Counter
    if (this.hasCountLabelTarget) {
      this.countLabelTarget.textContent = `${n + 1} / ${total}`
    }

    // Prev/next button states
    if (this.hasPrevBtnTarget) this.prevBtnTarget.disabled = n === 0
    if (this.hasNextBtnTarget) this.nextBtnTarget.disabled = n === total - 1

    // Slide transition — outgoing
    if (prev !== n && this.slideTargets[prev]) {
      const outEl = this.slideTargets[prev]
      outEl.classList.remove("tour-slide-in")
      outEl.classList.add("tour-slide-out")
      this._at(450, () => {
        outEl.classList.remove("tour-slide-out")
      })
    }

    // Slide transition — incoming
    slideEl.classList.remove("tour-slide-out", "tour-slide-in")
    void slideEl.offsetWidth
    slideEl.classList.add("tour-slide-in")

    // Per-slide vignette animation
    const type = slideEl.dataset.tourSlideType
    this._enterSlide(slideEl, type)
  }

  // ── per-slide vignette animations ─────────────────────────────────────────

  _enterSlide(slideEl, type) {
    if (REDUCED) {
      // Reduced motion: reveal everything instantly
      slideEl.querySelectorAll(".tour-stage-el").forEach((el) => el.classList.add("on"))
      slideEl.querySelectorAll(".tour-doc-field").forEach((el) => { el.style.opacity = "1" })
      if (type === "intro") this._showRot(slideEl, 0)
      return
    }

    // Stagger all .tour-stage-el elements
    this._stagger(slideEl, ".tour-stage-el", 200, 180)

    switch (type) {
      case "intro":
        this._runIntro(slideEl)
        break
      case "inbox":
        this._runInbox(slideEl)
        break
      case "calendar":
        this._runCalendar(slideEl)
        break
      case "tasks":
        this._runTasks(slideEl)
        break
      case "docs":
        this._runDocs(slideEl)
        break
      // "more": nothing extra beyond stagger
    }
  }

  _runIntro(slideEl) {
    if (REDUCED) { this._showRot(slideEl, 0); return }
    let i = 0
    this._showRot(slideEl, 0)
    this._rotTimer = setInterval(() => {
      i = (i + 1) % 5
      this._showRot(slideEl, i)
    }, 2500)
  }

  _showRot(slideEl, n) {
    slideEl.querySelectorAll("[data-tour-rot-item]").forEach((el, j) => {
      el.classList.remove("tour-rot-in", "tour-rot-out")
      if (j === n) el.classList.add("tour-rot-in")
      else if (j === (n - 1 + 5) % 5) el.classList.add("tour-rot-out")
    })
  }

  _runInbox(slideEl) {
    // Chips pop in after the rows have staggered in
    slideEl.querySelectorAll("[data-tour-chip]").forEach((chip, i) => {
      this._at(1200 + i * 300, () => {
        chip.style.opacity = "1"
        chip.classList.add("animate-chip-pop")
      })
    })
    // Group counter ticks up to its max
    this._at(2200, () => {
      const pill = slideEl.querySelector("[data-tour-counter]")
      if (!pill) return
      const max = parseInt(pill.dataset.tourCounterMax || "12", 10)
      let n = 0
      const tick = () => {
        n += 4
        pill.textContent = String(Math.min(n, max))
        if (n < max) this._at(120, tick)
      }
      tick()
    })
  }

  _runCalendar(slideEl) {
    slideEl.querySelectorAll(".tour-morph-btn").forEach((btn) => {
      const delay = parseInt(btn.dataset.tourMorphAt || "3000", 10)
      this._at(delay, () => {
        btn.textContent = btn.dataset.tourMorphText || btn.textContent
        btn.classList.add("animate-win-pop")
      })
    })
  }

  _runTasks(slideEl) {
    this._at(2600, () => {
      const tick = slideEl.querySelector("[data-tour-tick]")
      const text = slideEl.querySelector("[data-tour-tick-text]")
      if (tick) { tick.classList.add("tour-tick-done"); tick.textContent = "✓" }
      if (text) { text.classList.add("line-through", "text-muted-foreground", "!font-normal"); text.classList.remove("font-[550]") }
    })
  }

  _runDocs(slideEl) {
    // Doc fields fade in one by one
    slideEl.querySelectorAll(".tour-doc-field").forEach((f, i) => {
      this._at(1500 + i * 260, () => {
        f.style.transition = "opacity 0.35s"
        f.style.opacity = "1"
      })
    })
    // Approve button morphs
    const approveBtns = slideEl.querySelectorAll(".tour-morph-btn")
    approveBtns.forEach((btn) => {
      const delay = parseInt(btn.dataset.tourMorphAt || "3600", 10)
      this._at(delay, () => {
        btn.textContent = btn.dataset.tourMorphText || btn.textContent
        btn.classList.add("animate-win-pop")
      })
    })
  }

  // ── reset ─────────────────────────────────────────────────────────────────

  _resetSlides() {
    this.slideTargets.forEach((slideEl) => {
      slideEl.classList.remove("tour-slide-in", "tour-slide-out")

      // Stage elements
      slideEl.querySelectorAll(".tour-stage-el").forEach((el) => el.classList.remove("on"))

      // Chips
      slideEl.querySelectorAll("[data-tour-chip]").forEach((chip) => {
        chip.classList.remove("animate-chip-pop")
        chip.style.opacity = "0"
      })

      // Group counter
      slideEl.querySelectorAll("[data-tour-counter]").forEach((p) => { p.textContent = "0" })

      // Morph buttons — restore original text
      slideEl.querySelectorAll(".tour-morph-btn").forEach((btn) => {
        if (btn.dataset.originalText) btn.textContent = btn.dataset.originalText
        btn.classList.remove("animate-win-pop")
      })

      // Tasks tick
      slideEl.querySelectorAll("[data-tour-tick]").forEach((t) => {
        t.classList.remove("tour-tick-done")
        t.textContent = ""
      })
      slideEl.querySelectorAll("[data-tour-tick-text]").forEach((t) => {
        t.classList.remove("line-through", "text-muted-foreground", "!font-normal")
      })

      // Doc fields
      slideEl.querySelectorAll(".tour-doc-field").forEach((f) => {
        f.style.transition = ""
        f.style.opacity = "0"
      })

      // Rotation items
      slideEl.querySelectorAll("[data-tour-rot-item]").forEach((el) => {
        el.classList.remove("tour-rot-in", "tour-rot-out")
      })
    })
  }

  // ── helpers ───────────────────────────────────────────────────────────────

  // Run a staggered add of class "on" to matching elements (for .tour-stage-el).
  _stagger(root, selector, base, step) {
    root.querySelectorAll(selector).forEach((el, i) => {
      this._at(base + i * step, () => el.classList.add("on"))
    })
  }

  // Schedule a callback (stored so _clearAll can cancel it).
  _at(ms, fn) {
    this._timers.push(setTimeout(fn, ms))
  }

  // Cancel all pending timers and the rotator interval.
  _clearAll() {
    this._timers.forEach(clearTimeout)
    this._timers = []
    if (this._rotTimer) { clearInterval(this._rotTimer); this._rotTimer = null }
  }

  _markSeen() {
    fetch(DISMISS_URL, {
      method: "POST",
      headers: { "X-CSRF-Token": this._csrf, Accept: "application/json" },
      keepalive: true
    }).catch(() => {})
  }

  get _csrf() { return document.querySelector('meta[name="csrf-token"]')?.content || "" }
}
