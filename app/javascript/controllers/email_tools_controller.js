import { Controller } from "@hotwired/stimulus"

export default class extends Controller {
  showTyping(event) {
    const list = document.getElementById("comments_list")
    if (!list) return

    // Don't add if already present
    if (document.getElementById("scout_typing")) return

    const typing = document.createElement("div")
    typing.id = "scout_typing"
    typing.className = "px-4 py-3"
    typing.innerHTML = `
      <div class="flex items-start gap-3">
        <div class="w-6 h-6 rounded-full bg-blue-100 text-blue-500 flex items-center justify-center flex-shrink-0 mt-0.5">
          <svg class="w-3.5 h-3.5" fill="currentColor" viewBox="0 0 24 24">
            <path fill-rule="evenodd" d="M9 4.5a.75.75 0 01.721.544l.813 2.846a3.75 3.75 0 002.576 2.576l2.846.813a.75.75 0 010 1.442l-2.846.813a3.75 3.75 0 00-2.576 2.576l-.813 2.846a.75.75 0 01-1.442 0l-.813-2.846a3.75 3.75 0 00-2.576-2.576l-2.846-.813a.75.75 0 010-1.442l2.846-.813A3.75 3.75 0 007.466 7.89l.813-2.846A.75.75 0 019 4.5zM18 1.5a.75.75 0 01.728.568l.258 1.036c.236.94.88 1.584 1.82 1.82l1.036.258a.75.75 0 010 1.456l-1.036.258c-.94.236-1.584.88-1.82 1.82l-.258 1.036a.75.75 0 01-1.456 0l-.258-1.036a2.625 2.625 0 00-1.82-1.82l-1.036-.258a.75.75 0 010-1.456l1.036-.258a2.625 2.625 0 001.82-1.82l.258-1.036A.75.75 0 0118 1.5zM16.5 15a.75.75 0 01.712.513l.394 1.183c.15.447.5.799.948.948l1.183.395a.75.75 0 010 1.422l-1.183.395c-.447.15-.799.5-.948.948l-.395 1.183a.75.75 0 01-1.422 0l-.395-1.183a1.5 1.5 0 00-.948-.948l-1.183-.395a.75.75 0 010-1.422l1.183-.395a1.5 1.5 0 00.948-.948l.395-1.183A.75.75 0 0116.5 15z" clip-rule="evenodd"/>
          </svg>
        </div>
        <div class="flex-1 min-w-0">
          <div class="flex items-center gap-2">
            <span class="text-[12px] font-semibold text-gray-900">Scout</span>
            <span class="text-[10px] text-gray-400">drafting</span>
          </div>
          <div class="mt-1 flex items-center gap-1">
            <span class="w-1.5 h-1.5 rounded-full bg-blue-400" style="animation: typingBounce 1.4s ease-in-out 0s infinite both"></span>
            <span class="w-1.5 h-1.5 rounded-full bg-blue-400" style="animation: typingBounce 1.4s ease-in-out 0.2s infinite both"></span>
            <span class="w-1.5 h-1.5 rounded-full bg-blue-400" style="animation: typingBounce 1.4s ease-in-out 0.4s infinite both"></span>
          </div>
        </div>
      </div>
    `
    list.appendChild(typing)
    list.scrollTop = list.scrollHeight
  }
}
