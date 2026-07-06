import { Controller } from "@hotwired/stimulus"

// Manages the dynamic rules list in the inbox group settings form.
// Each rule row has a type selector and one of three value inputs
// (text, org select, doc-type select); the correct one is shown
// based on the selected type. Rows can be added (from a <template>)
// and removed without a server round-trip.
export default class extends Controller {
  static targets = ["list", "row", "template", "typeSelect", "textValue", "orgValue", "doctypeValue"]
  static values = { queryPlaceholder: String, senderPlaceholder: String }

  addRule() {
    const template = this.templateTarget.content.cloneNode(true)
    const idx = this.rowTargets.length

    // Replace the placeholder index in name attributes.
    template.querySelectorAll("[name]").forEach(el => {
      el.name = el.name.replace(/__IDX__/g, idx)
    })

    this.listTarget.appendChild(template)
    // Focus the type select of the newly added row.
    this.rowTargets[this.rowTargets.length - 1]?.querySelector("select")?.focus()
  }

  removeRule(event) {
    const row = event.currentTarget.closest("[data-group-rules-target='row']")
    row?.remove()
    this.reindexRows()
  }

  typeChanged(event) {
    const select = event.currentTarget
    const row = select.closest("[data-group-rules-target='row']")
    this.syncInputVisibility(row, select.value)
  }

  // Called by Stimulus when targets connect, so existing rows get their
  // initial visibility state wired up without an explicit typeChanged.
  typeSelectTargetConnected(selectEl) {
    const row = selectEl.closest("[data-group-rules-target='row']")
    if (row) this.syncInputVisibility(row, selectEl.value)
  }

  // ---- private ----

  syncInputVisibility(row, ruleType) {
    if (!row) return

    const textEl    = row.querySelector("[data-group-rules-target='textValue']")
    const orgEl     = row.querySelector("[data-group-rules-target='orgValue']")
    const doctypeEl = row.querySelector("[data-group-rules-target='doctypeValue']")

    const showText    = ruleType === "sender" || ruleType === "query"
    const showOrg     = ruleType === "organization"
    const showDoctype = ruleType === "document_type"

    this.setVisible(textEl, showText)
    this.setVisible(orgEl, showOrg)
    this.setVisible(doctypeEl, showDoctype)

    // Update placeholder to match type.
    if (textEl && ruleType === "query") {
      textEl.placeholder = this.queryPlaceholderValue
    } else if (textEl && ruleType === "sender") {
      textEl.placeholder = this.senderPlaceholderValue
    }

    // Disable hidden inputs so they don't submit a blank value for the
    // active input's name. The visible one remains enabled.
    ;[textEl, orgEl, doctypeEl].forEach((el, i) => {
      if (!el) return
      const visible = [showText, showOrg, showDoctype][i]
      el.disabled = !visible
    })
  }

  setVisible(el, visible) {
    if (!el) return
    el.classList.toggle("hidden", !visible)
  }

  // Re-number name attributes after a row is removed so indices stay contiguous.
  reindexRows() {
    this.rowTargets.forEach((row, idx) => {
      row.querySelectorAll("[name]").forEach(el => {
        el.name = el.name.replace(/rules\[\d+\]/g, `rules[${idx}]`)
      })
    })
  }
}
