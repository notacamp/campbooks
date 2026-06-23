import { Controller } from "@hotwired/stimulus"

// "Skim all" orchestrator (home only). The inbox skim and the document review are
// two separate full-screen overlays with different cards and actions; this stitches
// them into one continuous "Skim all" walk. The "Skim all" lead ring opens the inbox
// overlay and #arm records that documents remain. When the inbox viewer reaches its
// done-state it dispatches `skim-mode:completed`; we then close the inbox overlay and
// open the document overlay in the same tick — no extra tap, no flash of the inbox
// "all caught up". A manual dismiss dispatches `skim-overlay:closed`, which disarms,
// so a later single-ring skim never spills into documents.
//
// Both overlays are reached via outlets (document-wide selectors): the inbox overlay
// lives in the layout, the document overlay on this page. The chain only runs when a
// document leg actually exists (docCount > 0).
export default class extends Controller {
  static outlets = ["skim-overlay", "doc-skim-overlay"]
  static values = { docCount: Number }

  connect() {
    this.pendingDocs = false
    this.onCompleted = this.onCompleted.bind(this)
    this.onClosed = this.onClosed.bind(this)
    document.addEventListener("skim-mode:completed", this.onCompleted)
    document.addEventListener("skim-overlay:closed", this.onClosed)
  }

  disconnect() {
    document.removeEventListener("skim-mode:completed", this.onCompleted)
    document.removeEventListener("skim-overlay:closed", this.onClosed)
  }

  // Fired alongside the inbox overlay opening from the "Skim all" lead ring: arm the
  // hand-off so finishing the inbox stacks continues into documents. Stays disarmed
  // when there's nothing to review.
  arm() {
    this.pendingDocs = this.docCountValue > 0 && this.hasDocSkimOverlayOutlet
  }

  // The inbox viewer finished its last stack. If a document leg is armed, swap the
  // inbox overlay for the document overlay in one motion. Order matters: disarm and
  // close first (close re-enters via onClosed, which is then a no-op), then open.
  onCompleted() {
    if (!this.pendingDocs) return
    this.pendingDocs = false
    if (this.hasSkimOverlayOutlet) this.skimOverlayOutlet.close()
    if (this.hasDocSkimOverlayOutlet) this.docSkimOverlayOutlet.open()
  }

  // Inbox overlay dismissed (completed hand-off, Escape, or backdrop): always
  // disarm, so a stale arm can't later carry a single-ring skim into documents.
  onClosed() {
    this.pendingDocs = false
  }
}
