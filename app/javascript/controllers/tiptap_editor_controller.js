import { Controller } from "@hotwired/stimulus"
import { Editor } from "@tiptap/core"
import StarterKit from "@tiptap/starter-kit"
import Placeholder from "@tiptap/extension-placeholder"
import Underline from "@tiptap/extension-underline"
import Link from "@tiptap/extension-link"
import Image from "@tiptap/extension-image"
import TextAlign from "@tiptap/extension-text-align"
import TextStyle from "@tiptap/extension-text-style"
import { Color } from "@tiptap/extension-color"

// Rich-text editor backing every compose surface (reply drawer, new-message
// page) and the signature editor. The markup + toolbar live in
// Campbooks::RichTextEditor so there is one source of truth; this controller
// owns behaviour: it mounts TipTap, mirrors the HTML into a hidden <input> on
// every change, reflects the active marks/nodes back onto the toolbar buttons,
// and drives the link/image popovers + image upload.
export default class extends Controller {
  static targets = [
    "editor", "input", "toolbar", "blockType", "colorInput",
    "linkPopover", "linkInput", "imagePopover", "imageInput", "imageError", "fileInput"
  ]
  static values = {
    content: String,
    placeholder: { type: String, default: "Write something…" },
    toolbar: { type: Boolean, default: true },
    heading: { type: Boolean, default: true },
    codeBlock: { type: Boolean, default: true },
    blockquote: { type: Boolean, default: true },
    uploadUrl: String
  }

  connect() {
    const extensions = [
      StarterKit.configure({
        heading: this.headingValue ? { levels: [1, 2, 3] } : false,
        codeBlock: this.codeBlockValue,
        blockquote: this.blockquoteValue
        // horizontalRule stays enabled (StarterKit default)
      }),
      Placeholder.configure({ placeholder: this.placeholderValue }),
      Underline,
      Link.configure({
        openOnClick: false,
        autolink: true,
        linkOnPaste: true,
        HTMLAttributes: { rel: "noopener noreferrer nofollow", target: "_blank" }
      }),
      Image.configure({ inline: false, allowBase64: true, HTMLAttributes: { class: "rte-img" } }),
      TextAlign.configure({ types: ["heading", "paragraph"] }),
      TextStyle,
      Color
    ]

    this.editor = new Editor({
      element: this.editorTarget,
      extensions,
      content: this.contentValue || "",
      onUpdate: () => this._sync(),
      onSelectionUpdate: () => this._refreshToolbar(),
      onTransaction: () => this._refreshToolbar()
    })

    if (this.hasToolbarTarget) {
      this.toolbarTarget.classList.toggle("hidden", !this.toolbarValue)
    }

    // Image paste / drag-drop → upload (only when an upload endpoint is wired)
    if (this.uploadUrlValue) {
      this._onPaste = this._handlePaste.bind(this)
      this._onDrop = this._handleDrop.bind(this)
      this.editorTarget.addEventListener("paste", this._onPaste)
      this.editorTarget.addEventListener("drop", this._onDrop)
    }

    // Dismiss popovers on outside click
    this._onDocClick = this._handleDocClick.bind(this)
    document.addEventListener("click", this._onDocClick)

    this._sync()
    this._refreshToolbar()
  }

  disconnect() {
    if (this._onPaste) this.editorTarget.removeEventListener("paste", this._onPaste)
    if (this._onDrop) this.editorTarget.removeEventListener("drop", this._onDrop)
    if (this._onDocClick) document.removeEventListener("click", this._onDocClick)
    this.editor?.destroy()
  }

  // ── Programmatic API (used by Scout / compose-chat) ─────────
  appendContent(html) {
    this.editor?.commands.insertContent(html)
    this._sync()
  }

  setContent(html) {
    this.editor?.commands.setContent(html)
    this._sync()
  }

  getHTML() {
    return this.editor?.getHTML() || ""
  }

  // ── Inline marks ────────────────────────────────────────────
  toggleBold()      { this.editor?.chain().focus().toggleBold().run() }
  toggleItalic()    { this.editor?.chain().focus().toggleItalic().run() }
  toggleUnderline() { this.editor?.chain().focus().toggleUnderline().run() }
  toggleStrike()    { this.editor?.chain().focus().toggleStrike().run() }
  toggleCode()      { this.editor?.chain().focus().toggleCode().run() }

  // ── Block type ──────────────────────────────────────────────
  setBlockType(event) {
    const value = event.target.value
    const chain = this.editor?.chain().focus()
    if (!chain) return
    if (value === "paragraph") chain.setParagraph().run()
    else chain.setHeading({ level: parseInt(value.slice(1), 10) }).run()
  }

  toggleBulletList()  { this.editor?.chain().focus().toggleBulletList().run() }
  toggleOrderedList() { this.editor?.chain().focus().toggleOrderedList().run() }
  toggleBlockquote()  { this.editor?.chain().focus().toggleBlockquote().run() }
  toggleCodeBlock()   { this.editor?.chain().focus().toggleCodeBlock().run() }
  setHorizontalRule() { this.editor?.chain().focus().setHorizontalRule().run() }

  // ── Alignment ───────────────────────────────────────────────
  setAlign(event) {
    this.editor?.chain().focus().setTextAlign(event.params.align).run()
  }

  // ── Color ───────────────────────────────────────────────────
  setColor(event) {
    this.editor?.chain().focus().setColor(event.target.value).run()
  }

  // ── History / clear ─────────────────────────────────────────
  undo() { this.editor?.chain().focus().undo().run() }
  redo() { this.editor?.chain().focus().redo().run() }
  clearFormatting() { this.editor?.chain().focus().unsetAllMarks().clearNodes().run() }

  // ── Link popover ────────────────────────────────────────────
  openLink(event) {
    event?.preventDefault()
    if (!this.hasLinkPopoverTarget) return
    this._closePopovers()
    this.linkInputTarget.value = this.editor?.getAttributes("link").href || ""
    this.linkPopoverTarget.classList.add("is-open")
    requestAnimationFrame(() => this.linkInputTarget.focus())
  }

  applyLink(event) {
    event?.preventDefault()
    const url = this.linkInputTarget.value.trim()
    if (url === "") {
      this.editor?.chain().focus().extendMarkRange("link").unsetLink().run()
    } else {
      this.editor?.chain().focus().extendMarkRange("link").setLink({ href: this._normalizeUrl(url) }).run()
    }
    this.closeLink()
  }

  removeLink(event) {
    event?.preventDefault()
    this.editor?.chain().focus().extendMarkRange("link").unsetLink().run()
    this.closeLink()
  }

  closeLink() { this.linkPopoverTarget?.classList.remove("is-open") }

  linkKeydown(event) {
    if (event.key === "Enter") { event.preventDefault(); this.applyLink(event) }
    else if (event.key === "Escape") { event.preventDefault(); this.closeLink() }
  }

  // ── Image popover / upload ──────────────────────────────────
  openImage(event) {
    event?.preventDefault()
    if (!this.hasImagePopoverTarget) return
    this._closePopovers()
    this.imageInputTarget.value = ""
    this._clearImageError()
    this.imagePopoverTarget.classList.add("is-open")
    requestAnimationFrame(() => this.imageInputTarget.focus())
  }

  insertImageUrl(event) {
    event?.preventDefault()
    const url = this.imageInputTarget.value.trim()
    if (url) this.editor?.chain().focus().setImage({ src: this._normalizeUrl(url) }).run()
    this.closeImage()
  }

  imageKeydown(event) {
    if (event.key === "Enter") { event.preventDefault(); this.insertImageUrl(event) }
    else if (event.key === "Escape") { event.preventDefault(); this.closeImage() }
  }

  closeImage() { this.imagePopoverTarget?.classList.remove("is-open") }

  pickFile(event) {
    event?.preventDefault()
    this.fileInputTarget?.click()
  }

  async uploadFile(event) {
    const file = event.target.files?.[0]
    let ok = true
    if (file) ok = await this._uploadAndInsert(file)
    event.target.value = ""
    if (ok) this.closeImage()
  }

  // ── internals ───────────────────────────────────────────────
  _sync() {
    if (this.hasInputTarget && this.editor) this.inputTarget.value = this.editor.getHTML()
  }

  _refreshToolbar() {
    if (!this.hasToolbarTarget || !this.editor) return

    this.toolbarTarget.querySelectorAll("[data-rte-active]").forEach((btn) => {
      const spec = btn.dataset.rteActive
      let active = false
      try {
        active = spec.startsWith("{") ? this.editor.isActive(JSON.parse(spec)) : this.editor.isActive(spec)
      } catch (_e) { active = false }
      btn.classList.toggle("is-active", active)
    })

    if (this.hasBlockTypeTarget) {
      let value = "paragraph"
      for (const level of [1, 2, 3]) {
        if (this.editor.isActive("heading", { level })) { value = `h${level}`; break }
      }
      this.blockTypeTarget.value = value
    }

    if (this.hasColorInputTarget) {
      const color = this.editor.getAttributes("textStyle").color
      if (color) this.colorInputTarget.value = this._toHex(color)
    }
  }

  _normalizeUrl(url) {
    if (/^(https?:|mailto:|tel:|\/|#|data:)/i.test(url)) return url
    return `https://${url}`
  }

  _toHex(color) {
    const m = color.match(/^rgb\((\d+),\s*(\d+),\s*(\d+)\)$/i)
    if (!m) return color
    return "#" + [m[1], m[2], m[3]].map((n) => parseInt(n, 10).toString(16).padStart(2, "0")).join("")
  }

  _handlePaste(event) {
    const file = this._imageFromDataTransfer(event.clipboardData)
    if (file) { event.preventDefault(); this._uploadAndInsert(file) }
  }

  _handleDrop(event) {
    const file = this._imageFromDataTransfer(event.dataTransfer)
    if (file) { event.preventDefault(); this._uploadAndInsert(file) }
  }

  _imageFromDataTransfer(dt) {
    if (!dt) return null
    const items = Array.from(dt.files || [])
    return items.find((f) => f.type.startsWith("image/")) || null
  }

  // Uploads `file` and inserts the returned URL. Returns true on success; on
  // failure shows the (server-provided) message in the image popover.
  async _uploadAndInsert(file) {
    if (!this.uploadUrlValue) return true
    const body = new FormData()
    body.append("image", file)
    try {
      const res = await fetch(this.uploadUrlValue, {
        method: "POST",
        body,
        headers: { "X-CSRF-Token": this._csrf(), "Accept": "application/json" }
      })
      const data = await res.json().catch(() => ({}))
      if (!res.ok || !data.url) { this._showImageError(data.error); return false }
      this._clearImageError()
      this.editor?.chain().focus().setImage({ src: data.url, alt: data.alt || file.name }).run()
      return true
    } catch (_e) {
      this._showImageError()
      return false
    }
  }

  _showImageError(message) {
    if (!this.hasImageErrorTarget) return
    this.imageErrorTarget.textContent = message || "Upload failed"
    this.imageErrorTarget.classList.remove("hidden")
    this.imagePopoverTarget?.classList.add("is-open")
  }

  _clearImageError() {
    if (!this.hasImageErrorTarget) return
    this.imageErrorTarget.textContent = ""
    this.imageErrorTarget.classList.add("hidden")
  }

  _csrf() {
    return document.querySelector("meta[name='csrf-token']")?.content || ""
  }

  _handleDocClick(event) {
    if (!this.element.contains(event.target)) this._closePopovers()
  }

  _closePopovers() {
    this.linkPopoverTarget?.classList.remove("is-open")
    this.imagePopoverTarget?.classList.remove("is-open")
  }
}
