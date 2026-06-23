import { Controller } from "@hotwired/stimulus"
import { buildSegments, tint, showToast } from "controllers/skim_utils"

// Document Skim-as-Stories viewer. Steps through the review queue grouped into
// category "rings". The document-world analogue of skim_mode_controller.
//
// A approves the AI's classification (the Drive push is deferred so Undo can cancel
// it); → / tap-right / swipe-left skips to the next without changing anything; ← /
// tap-left / swipe-right goes back. C reclassify (grouped picker), E edit the key
// fields inline, R reprocess, J flag-as-junk, O open the full editor in a new tab.
// Approve and Junk show an Undo toast that returns the document to the queue.
//
// Per-frame regions (edit / reclassify panels, the open link) are plain data-*
// hooks queried within the current frame — not Stimulus targets — since there's
// one card per frame.
const UNDO_MS = 6000
const BASE = "/documents/skim"

export default class extends Controller {
  static targets = ["frame", "segments", "ringIcon", "roadmap", "roadmapChip", "roadmapSep", "done", "summary", "toast", "toastIcon", "toastMessage", "undo", "previewLayer", "previewBody", "previewTitle", "previewDownload", "previewOpen", "intro"]
  static values = { index: { type: Number, default: 0 } }

  connect() {
    this.approvedCount = 0
    this.lastAction = null // { type: "approve" | "dismiss", id }
    this.touch = null
    this.undoTimer = null
    this.render()
  }

  disconnect() { clearTimeout(this.undoTimer) }

  // ---- input ---------------------------------------------------------------

  onKeydown(event) {
    // While the first-run intro is up, nav is frozen; Enter/Space/Escape start.
    if (this.introOpen) {
      if (["Enter", " ", "Spacebar", "Escape"].includes(event.key)) { this.dismissIntro(); event.preventDefault(); event.stopPropagation() }
      return
    }
    // Escape closes the lightbox, then any open panel, before bubbling to the overlay.
    if (event.key === "Escape") {
      if (this.isPreviewOpen) { this.closePreview(); event.stopPropagation(); event.preventDefault(); return }
      if (this.panelOpen) { this.closePanels(); event.stopPropagation(); event.preventDefault(); return }
    }
    // Never hijack real typing (the edit inputs / reclassify select).
    const tag = (event.target.tagName || "").toLowerCase()
    if (tag === "input" || tag === "textarea" || tag === "select" || event.target.isContentEditable) return
    // While the lightbox is open, Space closes it; everything else is suppressed.
    if (this.isPreviewOpen) {
      if (event.key === " " || event.key === "Spacebar") { this.closePreview(); event.preventDefault() }
      return
    }
    // While a panel is open the action shortcuts are suppressed (commit or cancel first).
    if (this.panelOpen) return

    switch (event.key) {
      case "ArrowRight": this.next(); break
      case "ArrowLeft":  this.prev(); break
      case " ": case "Spacebar": this.openPreview(); break
      case "a": case "A": this.approve(); break
      case "c": case "C": this.openReclassify(); break
      case "e": case "E": this.openEdit(); break
      case "r": case "R": this.reprocess(); break
      case "j": case "J": this.dismiss(); break
      case "o": case "O": this.openDocument(); break
      default: return // let Escape etc. bubble up to the overlay (close)
    }
    event.preventDefault()
  }

  // Card buttons (data-doc-skim-action) bubble up to here.
  onClick(event) {
    if (this.introOpen) return
    const el = event.target.closest("[data-doc-skim-action]")
    if (!el) return
    switch (el.dataset.docSkimAction) {
      case "preview":          this.openPreview(); break
      case "approve":          this.approve(); break
      case "skip":             this.next(); break
      case "reclassify":       this.openReclassify(); break
      case "applyReclassify":  this.applyReclassify(); break
      case "cancelReclassify": this.closeReclassify(); break
      case "edit":             this.openEdit(); break
      case "saveFields":       this.saveFields(); break
      case "cancelEdit":       this.closeEdit(); break
      case "reprocess":        this.reprocess(); break
      case "junk":             this.dismiss(); break
    }
  }

  onTouchStart(event) {
    if (this.panelOpen || this.isPreviewOpen || this.introOpen) return
    const t = event.changedTouches[0]
    this.touch = { x: t.clientX, y: t.clientY }
  }

  onTouchEnd(event) {
    if (!this.touch) return
    const t = event.changedTouches[0]
    const dx = t.clientX - this.touch.x
    const dy = t.clientY - this.touch.y
    this.touch = null
    if (Math.abs(dx) > 40 && Math.abs(dx) > Math.abs(dy)) {
      if (dx < 0) this.next() // swipe content left → skip
      else this.prev()        // swipe right → back
    }
  }

  // ---- navigation ----------------------------------------------------------

  // Skip: advance without touching the document (it stays in the review queue).
  next() { this.advance() }
  advance() { if (this.indexValue < this.frameTargets.length) this.indexValue += 1 }
  prev() { if (this.indexValue > 0) this.indexValue -= 1 }

  // ---- per-document actions ------------------------------------------------

  approve() {
    const id = this.currentId
    if (!id) return
    this.lastAction = { type: "approve", id }
    this.approvedCount += 1
    this.advance() // optimistic — it's handled
    this.post(`${BASE}/${id}/approve`)
      .then(() => { this.flash("Approved", true); if (this.isDone) this.renderDone() })
      .catch(() => { this.approvedCount = Math.max(0, this.approvedCount - 1); this.flash("Couldn't approve — try again", false, false) })
  }

  dismiss() {
    const id = this.currentId
    if (!id) return
    this.lastAction = { type: "dismiss", id }
    this.advance()
    this.post(`${BASE}/${id}/dismiss`)
      .then(() => this.flash("Flagged as junk", true))
      .catch(() => this.flash("Couldn't flag — try again", false, false))
  }

  reprocess() {
    const id = this.currentId
    if (!id) return
    this.lastAction = null
    this.advance()
    this.post(`${BASE}/${id}/reprocess`)
      .then(() => this.flash("Re-analysing with AI…", false))
      .catch(() => this.flash("Couldn't reprocess — try again", false, false))
  }

  // Undo: return the just-actioned document to the review queue (works for both
  // approve and junk — the server's #restore reverses either).
  undoLast() {
    const last = this.lastAction
    if (!last) return
    this.hideToast()
    this.post(`${BASE}/${last.id}/restore`).then(() => {
      if (last.type === "approve") this.approvedCount = Math.max(0, this.approvedCount - 1)
      this.lastAction = null
      if (this.isDone) this.renderDone()
    })
  }

  // ---- reclassify ----------------------------------------------------------

  openReclassify() {
    const panel = this.currentPanel("reclassify")
    if (!panel) return
    this.closeEdit()
    panel.classList.remove("hidden")
    panel.querySelector("[data-doc-skim-reclassify-select]")?.focus()
  }

  closeReclassify() { this.currentPanel("reclassify")?.classList.add("hidden") }

  applyReclassify() {
    const id = this.currentId
    const select = this.currentPanel("reclassify")?.querySelector("[data-doc-skim-reclassify-select]")
    if (!id || !select) return
    const typeId = select.value
    this.closeReclassify()
    // Re-filing signs the document off (server marks it approved), so it counts
    // toward the session total and gets the same undoable toast as Approve.
    this.lastAction = { type: "approve", id }
    this.approvedCount += 1
    this.advance()
    this.patch(`${BASE}/${id}/reclassify`, { document_type_id: typeId })
      .then((d) => { this.flash(`Re-filed as ${d.type_label || "new type"}`, true); if (this.isDone) this.renderDone() })
      .catch(() => { this.approvedCount = Math.max(0, this.approvedCount - 1); this.flash("Couldn't reclassify — try again", false, false) })
  }

  // ---- inline field edit ---------------------------------------------------

  // The "Extracted data" fields live in a <details> that's collapsed by default;
  // E (or clicking the summary) expands it. Opening focuses the first input so you
  // can start correcting immediately.
  openEdit() {
    const fields = this.currentFields
    if (!fields) return
    this.closeReclassify()
    fields.open = true
    fields.querySelector("[data-doc-skim-field],[data-doc-skim-meta-field]")?.focus()
  }

  closeEdit() {
    const fields = this.currentFields
    if (fields) fields.open = false
  }

  saveFields() {
    const id = this.currentId
    const panel = this.currentFields
    if (!id || !panel) return
    // Top-level document fields (the display name) and the AI-extracted fields, which
    // nest under metadata so the server merges them into the metadata hash.
    const fields = {}
    panel.querySelectorAll("[data-doc-skim-field]").forEach((input) => {
      fields[input.dataset.docSkimField] = input.value
    })
    const metadata = {}
    panel.querySelectorAll("[data-doc-skim-meta-field]").forEach((input) => {
      metadata[input.dataset.docSkimMetaField] = input.value
    })
    if (Object.keys(metadata).length) fields.metadata = metadata
    this.closeEdit()
    this.patch(`${BASE}/${id}/update_fields`, { document: fields })
      .then((d) => {
        const title = this.currentFrame?.querySelector("[data-doc-skim-title]")
        if (title && d?.display_title) title.textContent = d.display_title
        this.flash("Saved", false)
      })
      .catch(() => this.flash("Couldn't save — try again", false, false))
  }

  // ---- open full editor ----------------------------------------------------

  openDocument() { this.currentFrame?.querySelector("[data-doc-skim-open]")?.click() }

  // ---- inline preview lightbox ---------------------------------------------

  // Lazy-load the active frame's PDF iframe: set its src (from data-src) only once
  // it's the current frame, so a long queue never loads every PDF at once. Fade it
  // in over the placeholder on load.
  loadCurrentPreview() {
    const el = this.currentFrame?.querySelector("[data-doc-skim-preview-frame]")
    if (!el || el.getAttribute("src") || !el.dataset.src) return
    el.addEventListener("load", () => el.classList.remove("opacity-0"), { once: true })
    el.setAttribute("src", el.dataset.src)
  }

  // Reveal the full-screen lightbox with a large view of the current document.
  openPreview() {
    const id = this.currentId
    const type = this.currentFrame?.dataset.docSkimPreviewType || "none"
    if (!id || type === "none" || !this.hasPreviewLayerTarget || !this.hasPreviewBodyTarget) return

    const fileUrl = `/documents/${id}/file`
    const name = this.currentFrame?.dataset.docSkimFilename || ""
    const media = type === "image" ? document.createElement("img") : document.createElement("iframe")
    media.src = fileUrl
    if (type === "image") {
      media.alt = name
      media.className = "h-full w-full object-contain"
    } else {
      media.title = name || "Document preview"
      media.className = "h-full w-full bg-card"
    }
    this.previewBodyTarget.replaceChildren(media)

    if (this.hasPreviewTitleTarget) this.previewTitleTarget.textContent = name
    if (this.hasPreviewDownloadTarget) this.previewDownloadTarget.href = `${fileUrl}?disposition=attachment`
    if (this.hasPreviewOpenTarget) this.previewOpenTarget.href = fileUrl

    this.previewLayerTarget.classList.remove("hidden")
    this.previewLayerTarget.classList.add("flex")
  }

  closePreview() {
    if (!this.hasPreviewLayerTarget) return
    this.previewLayerTarget.classList.add("hidden")
    this.previewLayerTarget.classList.remove("flex")
    if (this.hasPreviewBodyTarget) this.previewBodyTarget.replaceChildren() // stop loading
  }

  get isPreviewOpen() {
    return this.hasPreviewLayerTarget && !this.previewLayerTarget.classList.contains("hidden")
  }

  // ---- current-frame helpers ----------------------------------------------

  get currentFrame() { return this.frameTargets[this.indexValue] || null }
  get currentId() { return this.currentFrame?.dataset.docSkimId || null }
  get currentFields() { return this.currentFrame?.querySelector("[data-doc-skim-fields]") || null }
  currentPanel(kind) { return this.currentFrame?.querySelector(`[data-doc-skim-${kind}-panel]`) || null }

  // An open fields disclosure (you're editing) or an open reclassify panel both
  // suppress the nav/action shortcuts so typing/picking never triggers them.
  get panelOpen() {
    const f = this.currentFrame
    if (!f) return false
    if (this.currentFields?.open) return true
    return [...f.querySelectorAll("[data-doc-skim-reclassify-panel]")]
      .some((p) => !p.classList.contains("hidden"))
  }

  closePanels() { this.closeEdit(); this.closeReclassify() }

  // ---- requests ------------------------------------------------------------

  post(url) { return this.request("POST", url, null) }
  patch(url, body) { return this.request("PATCH", url, body) }

  request(method, url, body) {
    const opts = { method, headers: { "X-CSRF-Token": this.csrf, "Accept": "application/json" } }
    if (body) {
      opts.headers["Content-Type"] = "application/json"
      opts.body = JSON.stringify(body)
    }
    return fetch(url, opts).then((r) => (r.ok ? r.json() : Promise.reject(r)))
  }

  get csrf() { return document.querySelector('meta[name="csrf-token"]')?.content || "" }

  // ---- first-run intro -----------------------------------------------------

  get introOpen() { return this.hasIntroTarget && !this.introTarget.classList.contains("hidden") }

  // "Start": hide the explainer, hand keyboard control back to the stack, and
  // record the tour as seen so it won't greet this user again. Idempotent.
  dismissIntro(event) {
    if (event) event.preventDefault()
    if (!this.introOpen) return
    this.introTarget.classList.add("hidden")
    this.introTarget.classList.remove("flex")
    this.element.focus({ preventScroll: true })
    const key = this.introTarget.dataset.tourKey
    if (key) this.markTourSeen(key)
  }

  // Re-open the explainer (header "?"), e.g. for someone who skipped it.
  showIntro(event) {
    if (event) event.preventDefault()
    if (!this.hasIntroTarget) return
    this.introTarget.classList.remove("hidden")
    this.introTarget.classList.add("flex")
  }

  markTourSeen(key) {
    fetch(`/tours/${encodeURIComponent(key)}/dismiss`, {
      method: "POST",
      headers: { "X-CSRF-Token": this.csrf, "Accept": "application/json" }
    }).catch(() => {})
  }

  // ---- toast ---------------------------------------------------------------

  flash(text, undoable, success = true) {
    clearTimeout(this.undoTimer)
    this.undoTimer = showToast({
      toast: this.hasToastTarget ? this.toastTarget : null,
      messageEl: this.hasToastMessageTarget ? this.toastMessageTarget : null,
      icon: this.hasToastIconTarget ? this.toastIconTarget : null,
      undo: this.hasUndoTarget ? this.undoTarget : null,
      text, undoable, success, durationMs: UNDO_MS
    })
  }

  hideToast() {
    clearTimeout(this.undoTimer)
    if (!this.hasToastTarget) return
    this.toastTarget.classList.add("hidden")
    this.toastTarget.classList.remove("flex")
  }

  // ---- render --------------------------------------------------------------

  get isDone() { return this.frameTargets.length === 0 || this.indexValue >= this.frameTargets.length }

  indexValueChanged() { this.render() }

  render() {
    this.closePreview() // navigating away dismisses any open lightbox
    this.layoutFrames()
    if (this.hasDoneTarget) {
      this.doneTarget.classList.toggle("hidden", !this.isDone)
      this.doneTarget.classList.toggle("flex", this.isDone)
    }
    if (this.isDone) { tint(this.element, 260); this.setHeaderIcon(null); this.renderDone(); return }

    const frame = this.frameTargets[this.indexValue]
    this.resetFrame(frame)
    this.loadCurrentPreview()
    const pos = parseInt(frame.dataset.docSkimPos, 10) || 1
    const ringTotal = parseInt(frame.dataset.docSkimTotal, 10) || 1
    const hue = parseInt(frame.dataset.docSkimHue, 10) || 260
    buildSegments(this.hasSegmentsTarget ? this.segmentsTarget : null, ringTotal, pos, hue)
    tint(this.element, hue)
    this.setHeaderIcon(frame.dataset.docSkimIcon)
    this.updateRoadmap(parseInt(frame.dataset.docSkimRingIndex, 10) || 0, pos)
  }

  // Roadmap of category rings in the header: emphasise the current bucket (with its
  // live position), hide the ones already done, and leave the rest faded — so the
  // categories still approaching stay visible next to the current one, scrolled to
  // the left.
  updateRoadmap(currentRingIndex, pos) {
    if (!this.hasRoadmapChipTarget) return
    this.roadmapChipTargets.forEach((chip) => {
      const idx = parseInt(chip.dataset.docSkimChipIndex, 10)
      const total = parseInt(chip.dataset.docSkimChipTotal, 10) || 1
      const isCurrent = idx === currentRingIndex
      chip.classList.toggle("hidden", idx < currentRingIndex)
      chip.classList.toggle("font-semibold", isCurrent)
      chip.classList.toggle("text-foreground", isCurrent)
      chip.classList.toggle("font-medium", !isCurrent)
      chip.classList.toggle("text-muted-foreground/40", !isCurrent)
      const counter = chip.querySelector("[data-doc-skim-roadmap-counter]")
      if (counter) {
        counter.textContent = isCurrent ? `${pos} / ${total}` : String(total)
        counter.classList.toggle("rounded-full", isCurrent)
        counter.classList.toggle("bg-foreground/10", isCurrent)
        counter.classList.toggle("text-muted-foreground", isCurrent)
        counter.classList.toggle("text-muted-foreground/40", !isCurrent)
      }
    })
    // Chevrons show only ahead of the current bucket (never before the leftmost one).
    this.roadmapSepTargets.forEach((sep) => {
      sep.classList.toggle("hidden", parseInt(sep.dataset.docSkimSepIndex, 10) <= currentRingIndex)
    })
    if (this.hasRoadmapTarget) this.roadmapTarget.scrollLeft = 0
  }

  // Each card shows fresh when navigated to: the fields disclosure collapsed and the
  // reclassify panel closed.
  resetFrame(frame) {
    const fields = frame.querySelector("[data-doc-skim-fields]")
    if (fields) fields.open = false
    frame.querySelector("[data-doc-skim-reclassify-panel]")?.classList.add("hidden")
  }

  setHeaderIcon(svg) {
    if (!this.hasRingIconTarget) return
    this.ringIconTarget.innerHTML = svg || ""
    this.ringIconTarget.classList.toggle("hidden", !svg)
  }

  layoutFrames() {
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches
    const trans = reduce ? "" : "transition:opacity .28s ease;"
    this.frameTargets.forEach((f, i) => {
      if (i === this.indexValue) {
        f.classList.remove("hidden")
        f.style.cssText = trans + "opacity:1;z-index:10;pointer-events:auto;"
      } else {
        f.classList.add("hidden")
        f.style.cssText = trans + "opacity:0;pointer-events:none;"
      }
    })
  }

  renderDone() {
    const n = this.approvedCount
    if (this.hasSummaryTarget) {
      this.summaryTarget.textContent = n > 0
        ? `${n} document${n === 1 ? "" : "s"} approved. Nice work clearing the queue.`
        : "You went through the whole review queue."
    }
  }
}
