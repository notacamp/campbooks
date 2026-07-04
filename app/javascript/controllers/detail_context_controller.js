import { Controller } from "@hotwired/stimulus"

// Rendered inside the email_detail turbo frame's content (_detail_pane). The
// command palette (Cmd+K) and keyboard-shortcut context live as data attributes
// on <body>, which only a full page load refreshes — so after an in-frame email
// navigation they'd keep pointing at the previously open email. Connecting on
// every frame swap, this pushes the open email's id/subject/folders back up to
// <body>; Stimulus value observation propagates them into the live controllers.
export default class extends Controller {
  connect() {
    const { context, messageId, folders, subject } = this.element.dataset
    const body = document.body

    if (context) body.setAttribute("data-command-palette-context-value", context)
    if (messageId) {
      body.setAttribute("data-command-palette-message-id-value", messageId)
      body.setAttribute("data-email-shortcuts-message-id-value", messageId)
    }
    if (folders) body.setAttribute("data-command-palette-folders-value", folders)
    if (subject !== undefined) body.setAttribute("data-command-palette-subject-value", subject)
  }
}
