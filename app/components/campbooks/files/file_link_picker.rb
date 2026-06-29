# frozen_string_literal: true

module Campbooks
  module Files
    # Compose-toolbar affordance (Files Phase 3b): "Insert file link" opens a modal
    # that lazy-loads the workspace's files; picking one mints/reuses a public
    # FileShareLink and inserts an <a> into the TipTap body. Driven by the
    # file-link-picker Stimulus controller, which talks to the sibling tiptap-editor.
    class FileLinkPicker < Campbooks::Base
      def view_template
        div(class: "inline-block", data: { controller: "file-link-picker" }) do
          trigger_button
          dialog(
            data: { file_link_picker_target: "dialog", action: "click->file-link-picker#backdrop" },
            class: "m-auto w-[calc(100vw-2rem)] max-w-md overflow-hidden rounded-2xl border border-border bg-card p-0 text-card-foreground shadow-2xl backdrop:bg-black/40"
          ) do
            div(class: "flex items-center justify-between border-b border-border px-4 py-3") do
              h2(class: "text-sm font-semibold text-foreground") { t(".title") }
              close_button
            end
            p(class: "px-4 pt-3 text-xs text-muted-foreground") { t(".hint") }
            raw(safe(%(<turbo-frame id="file_link_picker_list" src="#{helpers.picker_files_public_links_path}" loading="lazy"><div class="px-4 py-8 text-center text-xs text-muted-foreground">#{t(".loading")}</div></turbo-frame>)))
          end
        end
      end

      private

      def trigger_button
        button(type: "button", data: { action: "file-link-picker#open" },
          class: "inline-flex items-center gap-1 px-3 py-1.5 text-xs text-gray-500 transition-colors hover:text-gray-700 dark:hover:text-gray-300") do
          raw(safe('<svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13.828 10.172a4 4 0 010 5.656l-3 3a4 4 0 11-5.656-5.656l1.5-1.5m6.328-1.328a4 4 0 010-5.656l3-3a4 4 0 115.656 5.656l-1.5 1.5"/></svg>'))
          plain t(".button")
        end
      end

      def close_button
        button(type: "button", data: { action: "file-link-picker#close" }, aria_label: t("shared.actions.close"),
          class: "cursor-pointer rounded p-1 text-gray-400 hover:bg-gray-200/70 hover:text-gray-600 dark:hover:bg-white/10") do
          raw(safe('<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/></svg>'))
        end
      end
    end
  end
end
