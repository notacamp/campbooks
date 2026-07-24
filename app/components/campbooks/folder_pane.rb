# frozen_string_literal: true

module Campbooks
  # The inbox's left-hand folder pane — a vertical, collapsible folder list that
  # replaces the horizontal chip bar on desktop (lg+). The `folder-pane`
  # controller toggles two states: an expanded panel (glyph + name + count) and a
  # slim icon-only rail. System folders (name-derived glyphs) sit above the
  # workspace's custom folders (their chosen glyph). Every row is a drag /
  # tap-to-move target (the shared `mail-folder-drop` controller, target value
  # "chip") except Sent/Drafts (see EmailFolder.droppable_name?).
  #
  # The "+" triggers the New Folder dialog via `new-folder#open`; the dialog +
  # `new-folder` controller live on a shared ancestor in the shell so there's a
  # single dialog across the pane and the mobile chip bar.
  #
  # @param system_folders [Array<Hash>] {id:, name:, count:} (from @common_folders)
  # @param custom_folders [Array<MailFolder>]
  # @param current_folder [String, Integer, nil] the active folder id / name
  class FolderPane < Campbooks::Base
    def initialize(system_folders:, custom_folders:, current_folder:, document_counts: {})
      @system = system_folders || []
      @custom = custom_folders || []
      @current = current_folder
      @document_counts = document_counts || {}
    end

    def view_template
      div(
        # No border-r / bg — the rail floats on the canvas, separated from the
        # thread list by whitespace alone (same vocabulary as the calendar sidebar).
        class: "hidden lg:flex flex-shrink-0",
        data: {
          controller: "folder-pane mail-folder-drop",
          action: "dragover->mail-folder-drop#dragover drop->mail-folder-drop#drop " \
                  "dragleave->mail-folder-drop#dragleave click->mail-folder-drop#click"
        }
      ) do
        expanded_panel
        collapsed_rail
      end
    end

    private

    def expanded_panel
      div(data: { folder_pane_target: "panel" }, class: "flex w-56 flex-col") do
        div(class: "flex h-10 flex-shrink-0 items-center justify-between pl-3 pr-2") do
          span(class: "text-[11px] font-semibold uppercase tracking-wide text-muted-foreground") { t(".folders") }
          rail_button(action: "folder-pane#collapse", label: t(".collapse"), path: "M11 19l-7-7 7-7")
        end
        div(class: "flex-1 space-y-0.5 overflow-y-auto px-2 pb-2") do
          @system.each do |f|
            expanded_row(name: f[:name], href: helpers.email_messages_path(folder_id: f[:id]),
                         count: f[:count], active: system_active?(f),
                         glyph: Campbooks::Icon.for_folder_name(f[:name]), droppable: EmailFolder.droppable_name?(f[:name]))
          end
          render(Campbooks::FolderPaneCustomFolders.new(custom_folders: @custom, current_folder: @current, document_counts: @document_counts))
          new_folder_button
        end
      end
    end

    def collapsed_rail
      div(data: { folder_pane_target: "rail" }, class: "hidden w-14 flex-col items-center gap-1 py-2") do
        rail_button(action: "folder-pane#expand", label: t(".expand"), path: "M13 5l7 7-7 7")
        @system.each do |f|
          collapsed_row(name: f[:name], href: helpers.email_messages_path(folder_id: f[:id]),
                        active: system_active?(f), glyph: Campbooks::Icon.for_folder_name(f[:name]),
                        droppable: EmailFolder.droppable_name?(f[:name]))
        end
        @custom.each do |f|
          collapsed_row(name: f.name, href: helpers.email_messages_path(folder_name: f.name),
                        active: @current == f.name, glyph: f.display_icon, droppable: true)
        end
        new_folder_button(collapsed: true)
      end
    end

    def expanded_row(name:, href:, count:, active:, glyph:, droppable:)
      a(href: href, data: row_data(name, active, droppable), class: row_classes(active)) do
        span(class: "flex h-5 w-5 flex-shrink-0 items-center justify-center") do
          render(Campbooks::Icon.new(glyph, css_class: "w-[18px] h-[18px]"))
        end
        span(class: "flex-1 truncate text-[13px]") { name }
        if count && count.to_i > 0
          span(class: count_classes(active)) { count.to_s }
        end
      end
    end

    def collapsed_row(name:, href:, active:, glyph:, droppable:)
      a(href: href, title: name, data: row_data(name, active, droppable),
        class: class_names(
          "flex h-9 w-9 items-center justify-center rounded-lg transition-colors",
          active ? "bg-accent-600 text-white" : "text-muted-foreground hover:bg-muted"
        )) do
        render(Campbooks::Icon.new(glyph, css_class: "w-5 h-5"))
      end
    end

    def new_folder_button(collapsed: false)
      if collapsed
        button(type: "button", data: { action: "new-folder#open" }, title: t(".new_folder"),
               aria_label: t(".new_folder"),
               class: "flex h-9 w-9 items-center justify-center rounded-lg text-muted-foreground hover:bg-muted cursor-pointer") do
          plus_icon
        end
      else
        button(type: "button", data: { action: "new-folder#open" },
               class: "mt-1 flex w-full items-center gap-2.5 rounded-lg px-2.5 py-2 text-[13px] text-muted-foreground hover:bg-muted hover:text-foreground cursor-pointer") do
          span(class: "flex h-5 w-5 items-center justify-center") { plus_icon }
          span { t(".new_folder") }
        end
      end
    end

    def row_data(name, active, droppable)
      data = { turbo_frame: "_top", folder_name: name, folder_active: active.to_s }
      data[:mail_folder_drop_target] = "chip" if droppable
      data
    end

    def row_classes(active)
      class_names(
        "flex items-center gap-2.5 rounded-lg px-2.5 py-1.5 transition-colors",
        active ? "bg-accent-50 font-medium text-accent-700 dark:bg-accent-500/15 dark:text-accent-200"
               : "text-gray-600 hover:bg-muted dark:text-gray-300"
      )
    end

    def count_classes(active)
      class_names(
        "ml-auto inline-flex h-4 min-w-[18px] items-center justify-center rounded-full px-1 text-[10px] font-bold",
        active ? "bg-accent-600 text-white" : "bg-muted text-muted-foreground"
      )
    end

    def system_active?(folder)
      (@current == folder[:id]) || (folder[:id].nil? && @current.nil?)
    end

    def rail_button(action:, label:, path:)
      button(type: "button", data: { action: action }, aria_label: label, title: label,
             class: "rounded-md p-1 text-muted-foreground hover:bg-muted cursor-pointer") do
        raw safe(%(<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="#{path}"/></svg>))
      end
    end

    def plus_icon
      raw safe('<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4"/></svg>')
    end
  end
end
