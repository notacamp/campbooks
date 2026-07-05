import { Controller } from "@hotwired/stimulus"

// Drives the "Report a bug" widget (Campbooks::BugReportModal). It is rendered
// once at the layout root and listens at the document level for clicks on any
// [data-bug-report-open] trigger, so the trigger can live in the nav while the
// full-screen overlay stays out of the nav's (blur-filtered) containing block.
//
// On open it snapshots page context + recent JS errors and kicks off an
// optional viewport screenshot (html2canvas, loaded lazily and excluded from
// the capture so the modal itself never appears). Submission goes over fetch;
// the plain <form> still works if JS is unavailable.
export default class extends Controller {
  static targets = [
    "overlay", "backdrop", "panel", "form", "formView", "successView",
    "description", "submit", "screenshotToggle", "error", "pageUrl", "metadata"
  ]
  static values = {
    submittingText: String,
    errorGeneric: String,
    errorEmpty: String
  }

  connect() {
    this.consoleErrors = []
    this.onError = (event) => this.recordError(event)
    this.onRejection = (event) => this.recordRejection(event)
    this.onTriggerClick = (event) => this.openFromTrigger(event)
    // Warm html2canvas (a CDN module) as soon as the user shows intent, so the
    // screenshot is ready by the time they finish typing and submit.
    this.onPrefetch = (event) => {
      if (event.target.closest && event.target.closest("[data-bug-report-open]")) {
        this.loadHtml2canvas().catch(() => {})
      }
    }
    window.addEventListener("error", this.onError)
    window.addEventListener("unhandledrejection", this.onRejection)
    document.addEventListener("click", this.onTriggerClick)
    document.addEventListener("pointerover", this.onPrefetch)
    document.addEventListener("focusin", this.onPrefetch)
  }

  disconnect() {
    window.removeEventListener("error", this.onError)
    window.removeEventListener("unhandledrejection", this.onRejection)
    document.removeEventListener("click", this.onTriggerClick)
    document.removeEventListener("pointerover", this.onPrefetch)
    document.removeEventListener("focusin", this.onPrefetch)
    if (this.closeTimeout) clearTimeout(this.closeTimeout)
    this.cancelScheduledScreenshot()
    document.body.style.overflow = ""
  }

  // Load (and cache) the screenshot library on demand. html2canvas-pro is a
  // maintained html2canvas fork that understands Tailwind v4's oklch() colors
  // (the original 1.4.1 throws on them).
  loadHtml2canvas() {
    return (this.libPromise ||= import("html2canvas-pro").then((mod) => mod.default))
  }

  // --- Open / close -------------------------------------------------------

  openFromTrigger(event) {
    if (!event.target.closest("[data-bug-report-open]")) return
    event.preventDefault()
    this.open()
  }

  open() {
    this.pageUrlTarget.value = window.location.href
    this.metadataTarget.value = JSON.stringify(this.collectMetadata())
    this.showFormView()
    // A reopen mid-close cancels the pending "hide after slide-out".
    if (this.closeTimeout) {
      clearTimeout(this.closeTimeout)
      this.closeTimeout = null
    }
    this.overlayTarget.classList.remove("hidden")
    document.body.style.overflow = "hidden"
    // Let the closed (off-screen) state paint, then flip to open on the next
    // frame so the drawer slides in instead of snapping into place.
    requestAnimationFrame(() => requestAnimationFrame(() => this.setOpenState(true)))
    setTimeout(() => this.descriptionTarget.focus(), 150)
    this.scheduleScreenshot()
  }

  close(event) {
    if (event) event.preventDefault()
    if (this.overlayTarget.classList.contains("hidden")) return
    this.setOpenState(false)
    document.body.style.overflow = ""
    // Hide (display:none) only once the slide-out has played, and only if a
    // quick reopen hasn't slid the panel back in meanwhile.
    const finish = () => {
      if (this.closeTimeout) {
        clearTimeout(this.closeTimeout)
        this.closeTimeout = null
      }
      if (!this.panelTarget.classList.contains("translate-x-full")) return
      this.overlayTarget.classList.add("hidden")
      if (this.hasFormTarget) this.formTarget.reset()
      this.showFormView()
      this.cancelScheduledScreenshot()
      this.screenshotBlob = null
      this.screenshotPromise = null
    }
    this.panelTarget.addEventListener("transitionend", finish, { once: true })
    this.closeTimeout = setTimeout(finish, 400)
  }

  // Toggle the slide (panel) + fade (backdrop) between open and closed.
  setOpenState(isOpen) {
    this.panelTarget.classList.toggle("translate-x-full", !isOpen)
    this.panelTarget.classList.toggle("translate-x-0", isOpen)
    this.backdropTarget.classList.toggle("opacity-0", !isOpen)
    this.backdropTarget.classList.toggle("opacity-100", isOpen)
  }

  // --- Submit -------------------------------------------------------------

  async submit(event) {
    event.preventDefault()

    const description = this.descriptionTarget.value.trim()
    if (!description) {
      this.showError(this.errorEmptyValue)
      this.descriptionTarget.focus()
      return
    }

    this.setSubmitting(true)
    this.clearError()

    const formData = new FormData()
    formData.append("description", description)
    formData.append("page_url", this.pageUrlTarget.value)
    formData.append("metadata", this.metadataTarget.value)

    if (this.hasScreenshotToggleTarget && this.screenshotToggleTarget.checked) {
      const blob = await this.resolveScreenshot()
      if (blob) formData.append("screenshot", blob, "screenshot.png")
    }

    try {
      const response = await fetch(this.formTarget.action, {
        method: "POST",
        headers: { "X-CSRF-Token": this.csrfToken(), Accept: "application/json" },
        body: formData,
        credentials: "same-origin"
      })
      if (!response.ok) throw new Error(`HTTP ${response.status}`)
      this.showSuccess()
    } catch (_error) {
      this.setSubmitting(false)
      this.showError(this.errorGenericValue)
    }
  }

  // --- Context capture ----------------------------------------------------

  collectMetadata() {
    return {
      viewport: `${window.innerWidth}x${window.innerHeight}`,
      screen: `${window.screen.width}x${window.screen.height}`,
      device_pixel_ratio: window.devicePixelRatio || 1,
      breakpoint: this.currentBreakpoint(),
      referrer: document.referrer || "",
      console_errors: this.consoleErrors.slice(-20),
      locale: document.documentElement.lang || ""
    }
  }

  currentBreakpoint() {
    const w = window.innerWidth
    if (w >= 1280) return "xl"
    if (w >= 1024) return "lg"
    if (w >= 768) return "md"
    if (w >= 640) return "sm"
    return "xs"
  }

  recordError(event) {
    const where = event.filename ? ` @ ${event.filename}:${event.lineno || 0}` : ""
    this.pushError(`${event.message || "Error"}${where}`)
  }

  recordRejection(event) {
    const reason = event.reason && event.reason.message ? event.reason.message : String(event.reason)
    this.pushError(`Unhandled rejection: ${reason}`)
  }

  pushError(message) {
    this.consoleErrors.push(String(message).slice(0, 500))
    if (this.consoleErrors.length > 50) this.consoleErrors.shift()
  }

  // --- Screenshot ---------------------------------------------------------

  // Kick off the capture only AFTER the drawer has slid in. html2canvas
  // deep-clones the whole page and forces thousands of style recalcs, all
  // synchronously on the main thread. The slide-in itself is compositor-driven,
  // but the class flip that STARTS it (setOpenState, via rAF) runs on the main
  // thread — so a capture on open blocks that flip and the drawer only appears
  // once html2canvas finishes (~300ms+), which is the "takes ages to show up".
  //
  // We defer with a macrotask timer that outlasts the ~300ms slide. NOT
  // requestIdleCallback: the compositor animation leaves the main thread idle,
  // so idle fires immediately (~2ms) and we'd be right back to blocking the
  // open. The timer can't fire early, guaranteeing the animation runs first.
  // The capture still finishes long before a user types a report and submits;
  // resolveScreenshot() starts one on demand if they somehow beat it. 450ms
  // clears the slide (~300ms transition + its ~2-frame start delay).
  scheduleScreenshot() {
    this.cancelScheduledScreenshot()
    this.screenshotBlob = null
    this.screenshotPromise = null
    this.screenshotTimer = setTimeout(() => {
      this.screenshotTimer = null
      // Bail if the drawer was closed before the capture got its turn.
      if (this.overlayTarget.classList.contains("hidden")) return
      if (!this.screenshotPromise) this.captureScreenshot()
    }, 450)
  }

  cancelScheduledScreenshot() {
    if (this.screenshotTimer) clearTimeout(this.screenshotTimer)
    this.screenshotTimer = null
  }

  captureScreenshot() {
    this.screenshotBlob = null
    this.screenshotPromise = this.takeScreenshot().catch(() => null)
  }

  async takeScreenshot() {
    const html2canvas = await this.loadHtml2canvas()
    const canvas = await html2canvas(document.body, {
      backgroundColor: getComputedStyle(document.body).backgroundColor || "#ffffff",
      scale: Math.min(window.devicePixelRatio || 1, 2),
      useCORS: true,
      logging: false,
      width: window.innerWidth,
      height: window.innerHeight,
      scrollX: -window.scrollX,
      scrollY: -window.scrollY,
      windowWidth: window.innerWidth,
      windowHeight: window.innerHeight,
      // Keep our own chrome out of the shot: the modal subtree and any trigger
      // (the nav buttons + the floating tab).
      ignoreElements: (element) =>
        this.element.contains(element) ||
        (typeof element.closest === "function" && element.closest("[data-bug-report-open]") !== null)
    })
    this.screenshotBlob = await new Promise((resolve) => canvas.toBlob(resolve, "image/png", 0.85))
    return this.screenshotBlob
  }

  // Don't let a slow/large capture block submission indefinitely.
  async resolveScreenshot() {
    // The deferred capture may not have fired yet (very fast submit) — the
    // screenshot is opt-in and requested, so kick one off now rather than skip.
    if (!this.screenshotPromise) this.captureScreenshot()
    try {
      return await Promise.race([
        this.screenshotPromise,
        new Promise((resolve) => setTimeout(() => resolve(this.screenshotBlob), 8000))
      ])
    } catch (_error) {
      return null
    }
  }

  // --- View state ---------------------------------------------------------

  showFormView() {
    this.successViewTarget.classList.add("hidden")
    this.formViewTarget.classList.remove("hidden")
    this.setSubmitting(false)
    this.clearError()
  }

  showSuccess() {
    this.formViewTarget.classList.add("hidden")
    this.successViewTarget.classList.remove("hidden")
  }

  setSubmitting(state) {
    if (!this.hasSubmitTarget) return
    this.submitTarget.disabled = state
    if (state) {
      if (!this.submitOriginalText) this.submitOriginalText = this.submitTarget.textContent
      this.submitTarget.textContent = this.submittingTextValue
    } else if (this.submitOriginalText) {
      this.submitTarget.textContent = this.submitOriginalText
    }
  }

  showError(message) {
    if (!this.hasErrorTarget) return
    this.errorTarget.textContent = message
    this.errorTarget.classList.remove("hidden")
  }

  clearError() {
    if (!this.hasErrorTarget) return
    this.errorTarget.textContent = ""
    this.errorTarget.classList.add("hidden")
  }

  csrfToken() {
    const meta = document.querySelector('meta[name="csrf-token"]')
    return meta ? meta.content : ""
  }
}
