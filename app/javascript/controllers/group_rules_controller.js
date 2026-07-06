import { Controller } from "@hotwired/stimulus"

// Manages the rules section in the inbox group form.
//
// UX model:
//   1. Committed rules render as readable sentence chips ("Sender is @acme.com [x]")
//      backed by hidden inputs that the server reads on submit.
//   2. "+ Add rule" reveals an add panel (type pills + value input + Add/Cancel).
//   3. "Add" validates the selection, builds a chip, closes the panel.
//   4. The "x" on any chip removes it and its hidden inputs.
//
// Sentence templates arrive from i18n via data values on the controller element
// so all four app languages work automatically.
export default class extends Controller {
  static targets = [
    "addPanel",      // the slide-in add panel (hidden by default)
    "chip",          // each committed rule chip (contains the hidden inputs)
    "chipsList",     // the flex-wrap wrapper that holds all chips
    "ruleTypeRadio", // radio inputs inside the type pill picker
    "senderGroup",   // wrapper div: sender text input + hint
    "orgGroup",      // wrapper div: org <select>
    "doctypeGroup",  // wrapper div: doctype <select> + color dot
    "queryGroup",    // wrapper div: query text input + hint
    "colorDot",      // the color swatch dot that lives inside doctypeGroup
  ]

  static values = {
    sentenceSender:       String, // "Sender is %{value}"
    sentenceOrganization: String, // "Organization is %{value}"
    sentenceDocumentType: String, // "Has a %{value} document"
    sentenceQuery:        String, // "Matches %{value}"
    removeLabel:          String, // "Remove rule" — aria-label for chip × button
  }

  // ---- public actions ----

  openPanel() {
    this.addPanelTarget.classList.remove("hidden")
    // Pre-select the first type pill if nothing is checked yet.
    if (!this.ruleTypeRadioTargets.some(r => r.checked)) {
      const first = this.ruleTypeRadioTargets[0]
      if (first) first.checked = true
    }
    this.syncValueArea()
    this.focusActiveInput()
  }

  closePanel() {
    this.addPanelTarget.classList.add("hidden")
    this.resetPanel()
  }

  typeChanged() {
    this.syncValueArea()
    this.focusActiveInput()
    this.updateColorDot()
  }

  // Allow pressing Enter in text inputs to commit without clicking Add.
  handleKeydown(event) {
    if (event.key === "Enter") {
      event.preventDefault()
      this.commitRule()
    }
  }

  commitRule() {
    const type = this.selectedType()
    if (!type) return

    const { value, label } = this.selectedValue(type)
    if (!value.trim()) {
      this.focusActiveInput()
      return
    }

    const sentence = this.buildSentence(type, label)
    const idx      = this.chipTargets.length
    const chip     = this.buildChip(type, value.trim(), sentence, idx)

    this.chipsListTarget.appendChild(chip)
    this.closePanel()
    this.reindex()
  }

  removeChip(event) {
    const chip = event.currentTarget.closest("[data-group-rules-target='chip']")
    chip?.remove()
    this.reindex()
  }

  // Stimulus callback: update color dot whenever the doctype select changes.
  doctypeSelectChanged() {
    this.updateColorDot()
  }

  // ---- private ----

  selectedType() {
    return this.ruleTypeRadioTargets.find(r => r.checked)?.value || null
  }

  selectedValue(type) {
    switch (type) {
      case "sender": {
        const input = this.hasSenderGroupTarget
          ? this.senderGroupTarget.querySelector("input")
          : null
        const v = input?.value?.trim() || ""
        return { value: v, label: v }
      }
      case "organization": {
        if (!this.hasOrgGroupTarget) return { value: "", label: "" }
        const sel = this.orgGroupTarget.querySelector("select")
        return {
          value: sel?.value || "",
          label: sel?.options[sel.selectedIndex]?.text || "",
        }
      }
      case "document_type": {
        if (!this.hasDoctypeGroupTarget) return { value: "", label: "" }
        const sel = this.doctypeGroupTarget.querySelector("select")
        return {
          value: sel?.value || "",
          label: sel?.options[sel.selectedIndex]?.text || "",
        }
      }
      case "query": {
        const input = this.hasQueryGroupTarget
          ? this.queryGroupTarget.querySelector("input")
          : null
        const v = input?.value?.trim() || ""
        return { value: v, label: v }
      }
      default:
        return { value: "", label: "" }
    }
  }

  buildSentence(type, label) {
    const templates = {
      sender:        this.sentenceSenderValue,
      organization:  this.sentenceOrganizationValue,
      document_type: this.sentenceDocumentTypeValue,
      query:         this.sentenceQueryValue,
    }
    return (templates[type] || "%{value}").replace("%{value}", label)
  }

  buildChip(type, value, sentence, idx) {
    const div = document.createElement("div")
    div.setAttribute("data-group-rules-target", "chip")
    div.className = "self-start inline-flex items-center gap-1.5 rounded-full " +
                    "border border-border bg-muted px-3 py-1 " +
                    "text-xs font-medium text-foreground/70"

    const rtInput = document.createElement("input")
    rtInput.type  = "hidden"
    rtInput.name  = `rules[${idx}][rule_type]`
    rtInput.value = type

    const rvInput = document.createElement("input")
    rvInput.type  = "hidden"
    rvInput.name  = `rules[${idx}][value]`
    rvInput.value = value

    const text = document.createElement("span")
    text.textContent = sentence

    const btn = document.createElement("button")
    btn.type  = "button"
    btn.setAttribute("data-action", "group-rules#removeChip")
    btn.setAttribute("aria-label", this.removeLabelValue)
    btn.className = "ml-0.5 -mr-0.5 flex items-center text-foreground/30 " +
                    "hover:text-foreground/70 transition-colors cursor-pointer"
    btn.innerHTML = `<svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5" aria-hidden="true">` +
                    `<path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12"/></svg>`

    div.append(rtInput, rvInput, text, btn)
    return div
  }

  syncValueArea() {
    const type = this.selectedType()
    const groups = {
      sender:        this.hasSenderGroupTarget  ? this.senderGroupTarget  : null,
      organization:  this.hasOrgGroupTarget     ? this.orgGroupTarget     : null,
      document_type: this.hasDoctypeGroupTarget ? this.doctypeGroupTarget : null,
      query:         this.hasQueryGroupTarget   ? this.queryGroupTarget   : null,
    }

    Object.entries(groups).forEach(([t, el]) => {
      if (!el) return
      const active = t === type
      el.classList.toggle("hidden", !active)
      const input = el.querySelector("input, select")
      if (input) input.disabled = !active
    })
  }

  focusActiveInput() {
    const type = this.selectedType()
    if (!type) return
    let target = null
    if (type === "sender" && this.hasSenderGroupTarget)
      target = this.senderGroupTarget.querySelector("input")
    else if (type === "query" && this.hasQueryGroupTarget)
      target = this.queryGroupTarget.querySelector("input")
    target?.focus()
  }

  updateColorDot() {
    if (!this.hasColorDotTarget || !this.hasDoctypeGroupTarget) return
    const sel   = this.doctypeGroupTarget.querySelector("select")
    const color = sel?.options[sel.selectedIndex]?.dataset?.color || ""
    if (color) {
      this.colorDotTarget.style.backgroundColor = color
      this.colorDotTarget.classList.remove("hidden")
    } else {
      this.colorDotTarget.classList.add("hidden")
    }
  }

  resetPanel() {
    this.ruleTypeRadioTargets.forEach(r => { r.checked = false })

    if (this.hasSenderGroupTarget) {
      const input = this.senderGroupTarget.querySelector("input")
      if (input) input.value = ""
    }
    if (this.hasQueryGroupTarget) {
      const input = this.queryGroupTarget.querySelector("input")
      if (input) input.value = ""
    }
    if (this.hasOrgGroupTarget) {
      const sel = this.orgGroupTarget.querySelector("select")
      if (sel) sel.selectedIndex = 0
    }
    if (this.hasDoctypeGroupTarget) {
      const sel = this.doctypeGroupTarget.querySelector("select")
      if (sel) sel.selectedIndex = 0
    }

    // Hide all value groups and the color dot.
    ;[
      this.hasSenderGroupTarget  && this.senderGroupTarget,
      this.hasOrgGroupTarget     && this.orgGroupTarget,
      this.hasDoctypeGroupTarget && this.doctypeGroupTarget,
      this.hasQueryGroupTarget   && this.queryGroupTarget,
    ].filter(Boolean).forEach(el => {
      el.classList.add("hidden")
      const input = el.querySelector("input, select")
      if (input) input.disabled = true
    })

    if (this.hasColorDotTarget) this.colorDotTarget.classList.add("hidden")
  }

  // Renumber rules[N][...] in all chip hidden inputs so indices stay contiguous
  // after a chip is removed or added.
  reindex() {
    this.chipTargets.forEach((chip, idx) => {
      chip.querySelectorAll("input[type='hidden']").forEach(input => {
        input.name = input.name.replace(/rules\[\d+\]/, `rules[${idx}]`)
      })
    })
  }
}
