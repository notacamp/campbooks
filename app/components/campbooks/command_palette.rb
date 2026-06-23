# frozen_string_literal: true

module Campbooks
  class CommandPalette < Campbooks::Base
    # `preview:` opens the dialog and renders representative rows so the design is
    # reviewable in Lookbook. `:search` (or true) shows the search/command list;
    # `:capture` shows a composite command mid-flow (breadcrumb + folder picker).
    # In the app the list is rendered live by the `command-palette` controller.
    def initialize(preview: false)
      @preview = preview
      @capture = (preview == :capture)
    end

    def view_template
      dialog(
        **(@preview ? { open: true } : {}),
        data: { command_palette_target: "dialog" },
        class: "command-palette-dialog w-full max-w-xl bg-card rounded-xl shadow-2xl border border-gray-200 p-0 overflow-hidden",
        aria: { label: t(".dialog_aria_label") }
      ) do
        div(class: "flex items-center gap-3 px-4 py-3 border-b border-gray-200") do
          render_search_icon
          render_breadcrumb
          input(
            type: "text",
            placeholder: (@capture ? t(".capture_placeholder") : t(".search_placeholder")),
            value: (@capture ? nil : (@preview ? "invoice" : nil)),
            role: "combobox",
            aria: { label: t(".search_aria_label"), expanded: "true", controls: "command-palette-results", autocomplete: "list" },
            data: { command_palette_target: "input", action: "input->command-palette#filter" },
            class: "flex-1 min-w-0 text-sm text-gray-900 placeholder-gray-400 bg-transparent border-0 outline-none focus:ring-0 p-0",
            autocomplete: "off",
            spellcheck: "false"
          )
        end

        div(
          id: "command-palette-results",
          role: "listbox",
          aria: { label: t(".results_aria_label") },
          # Click/hover are delegated here (the list innerHTML is rebuilt constantly;
          # per-row listeners can miss a click in the gap before Stimulus re-binds).
          data: {
            command_palette_target: "list",
            action: "click->command-palette#selectItem mouseover->command-palette#hoverItem"
          },
          class: "max-h-80 overflow-y-auto py-2"
        ) { render_preview_rows if @preview }

        div(class: "px-4 py-2 border-t border-gray-100 text-[10px] text-gray-400 flex items-center gap-3") do
          span { plain t(".hint_navigate") }
          span { plain t(".hint_select") }
          span { plain(@capture ? t(".hint_escape_back") : t(".hint_escape_close")) }
        end
      end
    end

    private

    # Capture-mode breadcrumb: filled by the Stimulus controller in the app; the
    # `:capture` preview seeds it with sample chips. Hidden (empty) otherwise.
    def render_breadcrumb
      unless @capture
        div(data: { command_palette_target: "breadcrumb" }, class: "hidden")
        return
      end

      div(data: { command_palette_target: "breadcrumb" }, class: "flex items-center gap-1.5 flex-shrink-0 min-w-0") do
        breadcrumb_chip(t(".move_email_to_folder"), accent: true)
        span(class: "text-gray-300 text-xs") { raw(safe("&rsaquo;")) }
        breadcrumb_chip("Re: Acme Invoice 0850012")
      end
    end

    def breadcrumb_chip(label, accent: false)
      tint = accent ? "bg-accent-50 text-accent-700 font-medium" : "bg-gray-100 text-gray-700"
      span(class: class_names("inline-flex items-center max-w-[160px] truncate px-2 py-0.5 rounded-md text-xs whitespace-nowrap", tint)) { label }
    end

    def render_preview_rows
      return render_capture_rows if @capture

      group_header(t(".group_navigate"))
      preview_row(ICON_MAIL, "Inbox")
      group_header(t(".group_actions"))
      preview_row(ICON_PEN, "Start new email")
      group_header(t(".group_emails"))
      preview_row(ICON_MAIL, "Your invoice from Apple.", "Apple <no_reply@email.apple.com>", selected: true)
      preview_row(ICON_MAIL, "Acme Cloud GmbH — Invoice 0850012", "user@example.com")
      group_header(t(".group_contacts"))
      preview_row(ICON_USERS, "Anna Schmidt", "anna@acme.com")
    end

    def render_capture_rows
      group_header(t(".group_folder"))
      preview_row(ICON_FOLDER, "Inbox")
      preview_row(ICON_FOLDER, "ACCOUNTING", selected: true)
      preview_row(ICON_FOLDER, "Receipts")
      preview_row(ICON_FOLDER, "Archive")
    end

    def group_header(label)
      div(class: "px-2 pt-2 pb-1 text-[10px] font-semibold text-gray-400 uppercase tracking-wider") { label }
    end

    def preview_row(icon, title, subtitle = nil, selected: false)
      button(
        type: "button",
        role: "option",
        aria: { selected: selected.to_s },
        class: class_names(
          "w-full flex items-center gap-3 px-3 py-2 text-left rounded-lg transition-colors",
          selected ? "bg-accent-50" : "hover:bg-gray-100"
        )
      ) do
        span(class: class_names("flex-shrink-0", selected ? "text-accent-600" : "text-gray-400")) { raw(safe(icon)) }
        span(class: "min-w-0 flex-1") do
          span(class: class_names("block truncate text-xs", selected ? "text-accent-700 font-medium" : "text-gray-700")) { title }
          if subtitle
            span(class: class_names("block truncate text-[11px]", selected ? "text-accent-600/80" : "text-gray-500")) { subtitle }
          end
        end
      end
    end

    def render_search_icon
      raw(safe('<svg class="w-4 h-4 text-gray-400 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"/></svg>'))
    end

    ICON_MAIL = '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 8l7.89 5.26a2 2 0 002.22 0L21 8M5 19h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z"/></svg>'
    ICON_USERS = '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4.354a4 4 0 110 5.292M15 21H3v-1a6 6 0 0112 0v1zm0 0h6v-1a6 6 0 00-9-5.197M13 7a4 4 0 11-8 0 4 4 0 018 0z"/></svg>'
    ICON_PEN = '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15.232 5.232l3.536 3.536m-2.036-5.036a2.5 2.5 0 113.536 3.536L6.5 21.036H3v-3.572L16.732 3.732z"/></svg>'
    ICON_FOLDER = '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"/></svg>'
  end
end
