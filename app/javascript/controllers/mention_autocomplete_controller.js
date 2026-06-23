import { Controller } from "@hotwired/stimulus"

// @mention autocomplete for the discussion composer. Suggests workspace
// teammates and Scout (the AI). Selecting inserts "@Name ".
//
// Shares the textarea with the chat-input controller. While the menu is open it
// swallows Enter / Tab / Arrow / Escape (stopImmediatePropagation) so a message
// isn't sent mid-selection; when closed it stays out of the way so Enter sends
// as usual. Its keydown action must be listed before chat-input's on the input.
export default class extends Controller {
  static targets = ["input", "menu"]
  static values = { candidates: Array }

  connect() {
    this.matches = []
    this.activeIndex = 0
    this.tokenStart = null
    this.isOpen = false
    this._onOutside = this.closeOnOutside.bind(this)
    document.addEventListener("click", this._onOutside)
  }

  disconnect() {
    document.removeEventListener("click", this._onOutside)
  }

  onInput() {
    const el = this.inputTarget
    const upToCursor = el.value.slice(0, el.selectionStart)
    // An @token at the cursor: '@' at line start or after whitespace, then name
    // characters with no space (a single word being typed).
    const match = upToCursor.match(/(?:^|\s)@([\p{L}\p{N}_'\-.]*)$/u)
    if (!match) return this.close()

    const query = match[1].toLowerCase()
    this.tokenStart = el.selectionStart - match[1].length - 1 // index of '@'
    this.matches = this.candidatesValue
      .filter((c) => c.name && c.name.toLowerCase().includes(query))
      .slice(0, 6)

    if (this.matches.length === 0) return this.close()
    this.activeIndex = 0
    this.render()
    this.open()
  }

  keydown(event) {
    if (!this.isOpen) return
    switch (event.key) {
      case "ArrowDown":
        event.preventDefault()
        event.stopImmediatePropagation()
        this.activeIndex = (this.activeIndex + 1) % this.matches.length
        this.render()
        break
      case "ArrowUp":
        event.preventDefault()
        event.stopImmediatePropagation()
        this.activeIndex = (this.activeIndex - 1 + this.matches.length) % this.matches.length
        this.render()
        break
      case "Enter":
      case "Tab":
        event.preventDefault()
        event.stopImmediatePropagation()
        this.select(this.matches[this.activeIndex])
        break
      case "Escape":
        event.preventDefault()
        event.stopImmediatePropagation()
        this.close()
        break
    }
  }

  select(candidate) {
    if (!candidate) return
    const el = this.inputTarget
    const before = el.value.slice(0, this.tokenStart)
    const after = el.value.slice(el.selectionStart)
    const insert = `@${candidate.name} `
    el.value = before + insert + after
    const caret = before.length + insert.length
    el.setSelectionRange(caret, caret)
    el.focus()
    this.close()
    // Let chat-input grow the textarea and enable the send button.
    el.dispatchEvent(new Event("input", { bubbles: true }))
  }

  render() {
    this.menuTarget.innerHTML = this.matches.map((c, i) => this.itemHtml(c, i)).join("")
    this.menuTarget.querySelectorAll("[data-mention-index]").forEach((node) => {
      // mousedown (not click) so it fires before the textarea blurs.
      node.addEventListener("mousedown", (event) => {
        event.preventDefault()
        this.select(this.matches[Number(node.dataset.mentionIndex)])
      })
    })
  }

  itemHtml(candidate, index) {
    const active = index === this.activeIndex ? "bg-muted" : ""
    const isScout = candidate.kind === "scout"
    const name = this.escape(candidate.name)
    const avatar = isScout
      ? `<span class="inline-flex items-center justify-center w-5 h-5 rounded-full bg-accent-500 text-white text-[10px] font-semibold flex-shrink-0">✦</span>`
      : `<span class="inline-flex items-center justify-center w-5 h-5 rounded-full bg-accent-100 text-accent-700 text-[10px] font-semibold flex-shrink-0">${this.initial(candidate.name)}</span>`
    const badge = isScout ? `<span class="ml-auto text-[10px] font-semibold text-accent-600 dark:text-accent-300">AI</span>` : ""
    return `<button type="button" data-mention-index="${index}" class="flex items-center gap-2 w-full text-left px-2.5 py-1.5 text-[13px] cursor-pointer hover:bg-muted ${active}">${avatar}<span class="text-foreground truncate">${name}</span>${badge}</button>`
  }

  initial(name) {
    return this.escape((name || "?").trim().charAt(0).toUpperCase())
  }

  escape(value) {
    const node = document.createElement("div")
    node.textContent = value == null ? "" : String(value)
    return node.innerHTML
  }

  open() {
    this.menuTarget.classList.remove("hidden")
    this.isOpen = true
  }

  close() {
    this.menuTarget.classList.add("hidden")
    this.isOpen = false
    this.matches = []
  }

  closeOnOutside(event) {
    if (!this.element.contains(event.target)) this.close()
  }
}
