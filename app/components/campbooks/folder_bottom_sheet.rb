# frozen_string_literal: true

module Campbooks
  # Mobile-only bottom sheet that surfaces the full folder list with edit
  # affordances, giving mobile users the same capabilities as the desktop
  # FolderPane sidebar. Rendered alongside the horizontal chip bar; the
  # chip bar's "Folders" trigger (data-action="folder-bottom-sheet#open")
  # slides it up.
  #
  # The sheet reuses FolderPaneCustomFolders (with section_id "sheet_custom_folders")
  # so MailFoldersController can keep it live via turbo streams alongside the
  # desktop pane's "pane_custom_folders". Hidden entirely on lg+ (desktop uses
  # FolderPane directly).
  #
  # @param system_folders [Array<Hash>] {id:, name:, count:}
  # @param custom_folders [Array<MailFolder>]
  # @param current_folder [String, Integer, nil] active folder id / name
  # @param document_counts [Hash] folder_id → count
  class FolderBottomSheet < Campbooks::Base
    CLOSE_SVG = '<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">' \
                '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/></svg>'
    PLUS_SVG  = '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">' \
                '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4"/></svg>'

    def initialize(system_folders:, custom_folders:, current_folder:, document_counts: {})
      @system          = system_folders || []
      @custom          = custom_folders || []
      @current         = current_folder
      @document_counts = document_counts || {}
    end

    def view_template
      # Backdrop — taps close the sheet
      div(
        class: "fixed inset-0 z-40 bg-black/40 backdrop-blur-[2px]",
        data: {
          folder_bottom_sheet_target: "backdrop",
          action: "click->folder-bottom-sheet#close"
        },
        style: "display: none"
      )

      # Sheet panel — slides up from below
      div(
        class: "fixed inset-x-0 bottom-0 z-50 invisible translate-y-full " \
               "flex flex-col bg-card rounded-t-[24px] shadow-2xl border-t border-border " \
               "transition-transform duration-300 ease-out max-h-[85svh]",
        role: "dialog",
        aria_modal: "true",
        aria_label: t("components.folder_pane.folders"),
        data: { folder_bottom_sheet_target: "panel" }
      ) do
        drag_handle
        sheet_header
        folder_list
      end
    end

    private

    def drag_handle
      div(class: "flex justify-center pt-3 pb-1 flex-shrink-0") do
        div(class: "w-10 h-1 rounded-full bg-border")
      end
    end

    def sheet_header
      div(class: "flex items-center justify-between px-5 py-3 border-b border-border flex-shrink-0") do
        h2(class: "text-[15px] font-semibold tracking-[-0.01em] text-foreground") do
          t("components.folder_pane.folders")
        end
        button(
          type: "button",
          aria_label: t("shared.actions.close"),
          class: "flex h-8 w-8 items-center justify-center rounded-lg text-muted-foreground " \
                 "hover:bg-muted hover:text-foreground transition-colors cursor-pointer",
          data: { action: "folder-bottom-sheet#close" }
        ) do
          raw safe(CLOSE_SVG)
        end
      end
    end

    def folder_list
      div(class: "flex-1 overflow-y-auto px-3 py-2 pb-6") do
        @system.each do |f|
          folder_row(
            name:      f[:name],
            # show_list=1 lands mobile taps on the thread list, not the reading pane
            # (see EmailMessagesController#index + email_mobile_controller); no-op on desktop.
            href:      helpers.email_messages_path(folder_id: f[:id], show_list: 1),
            count:     f[:count],
            active:    system_active?(f),
            glyph:     Campbooks::Icon.for_folder_name(f[:name]),
            droppable: EmailFolder.droppable_name?(f[:name])
          )
        end

        render(
          Campbooks::FolderPaneCustomFolders.new(
            custom_folders:  @custom,
            current_folder:  @current,
            document_counts: @document_counts,
            section_id:      "sheet_custom_folders",
            dom_prefix:      "sheet_",
            list_param:      true
          )
        )

        new_folder_button
      end
    end

    def folder_row(name:, href:, count:, active:, glyph:, droppable:)
      link_data = { turbo_frame: "_top", folder_name: name, folder_active: active.to_s }
      link_data[:mail_folder_drop_target] = "chip" if droppable

      a(href: href, data: link_data,
        class: class_names(
          "flex items-center gap-3 rounded-xl px-3 py-2.5 transition-colors",
          active ? "bg-accent-50 font-medium text-accent-700 dark:bg-accent-500/15 dark:text-accent-200"
                 : "text-gray-600 hover:bg-muted dark:text-gray-300"
        )) do
        span(class: "flex h-6 w-6 flex-shrink-0 items-center justify-center") do
          render(Campbooks::Icon.new(glyph, css_class: "w-5 h-5"))
        end
        span(class: "flex-1 truncate text-[14px]") { name }
        if count && count.to_i > 0
          span(class: class_names(
            "ml-auto inline-flex h-5 min-w-[20px] items-center justify-center rounded-full px-1.5 text-[11px] font-bold",
            active ? "bg-accent-600 text-white" : "bg-muted text-muted-foreground"
          )) { count.to_s }
        end
      end
    end

    def new_folder_button
      button(
        type: "button",
        data: { action: "new-folder#open" },
        class: "mt-2 flex w-full items-center gap-3 rounded-xl px-3 py-2.5 text-[14px] " \
               "text-muted-foreground hover:bg-muted hover:text-foreground transition-colors cursor-pointer"
      ) do
        span(class: "flex h-6 w-6 items-center justify-center") { raw safe(PLUS_SVG) }
        span { t("components.folder_pane.new_folder") }
      end
    end

    def system_active?(folder)
      (@current == folder[:id]) || (folder[:id].nil? && @current.nil?)
    end
  end
end
