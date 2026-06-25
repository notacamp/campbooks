import { Controller } from "@hotwired/stimulus"

// Opens a custom folder's per-row edit <dialog> (rendered inside
// #pane_custom_folders by FolderPaneCustomFolders). A save or delete returns a
// Turbo Stream that re-renders #pane_custom_folders, which removes the open
// dialog — so there's no explicit "close on success" to manage here.
export default class extends Controller {
  open(e) {
    const d = document.getElementById(e.params.dialog)
    if (d) d.showModal()
  }

  close(e) {
    e.target.closest("dialog")?.close()
  }

  // Native <dialog> backdrop click resolves to the dialog element itself.
  backdropClose(e) {
    if (e.target.tagName === "DIALOG") e.target.close()
  }
}
