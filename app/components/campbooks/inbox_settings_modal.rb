# frozen_string_literal: true

module Campbooks
  # The inbox "gear" settings dialog. A two-pane management surface: a left
  # vertical nav of sections and a content pane that lazy-loads each section
  # into a single Turbo Frame (`inbox_settings_panel`).
  #
  # Mounted once in the email layout (outside the `email_detail` frame). Opened
  # by the gear icon via the `inbox-settings-modal` Stimulus controller, which
  # also handles the localStorage-backed Display preferences and `?inbox_settings=`
  # deep-links.
  class InboxSettingsModal < Campbooks::Base
    # Section the modal opens to by default.
    DEFAULT_SECTION = "tags"

    def initialize(open: false, default_section: DEFAULT_SECTION, **attrs)
      @open = open
      @default_section = default_section
      @attrs = attrs
    end

    # Order matches the approved layout: management sections first, Display last.
    # Defined as a method so t() resolves at render time (not class-load).
    def nav_items
      [
        { key: "tags",           label: t(".nav.tags"),           path: :inbox_settings_tags_path,           icon: :tag },
        { key: "document_types", label: t(".nav.document_types"), path: :inbox_settings_document_types_path,  icon: :doc },
        { key: "filtering",      label: t(".nav.filtering"),      path: :inbox_settings_filtering_path,       icon: :filter },
        { key: "labels",         label: t(".nav.labels"),         path: :inbox_settings_external_labels_path, icon: :label },
        { key: "signatures",     label: t(".nav.signatures"),     path: :inbox_settings_signatures_path,      icon: :pen },
        { key: "accounts",       label: t(".nav.accounts"),       path: :inbox_settings_accounts_path,        icon: :at },
        { key: "display",        label: t(".nav.display"),        path: :inbox_settings_display_path,         icon: :sliders }
      ]
    end

    def view_template
      dialog(
        id: "inbox-settings-dialog",
        class: "rounded-2xl shadow-2xl border border-border p-0 overflow-hidden backdrop:bg-black/40 m-auto",
        aria: { label: t(".title") },
        data: {
          inbox_settings_modal_target: "dialog",
          action: "click->inbox-settings-modal#backdropClose"
        },
        open: @open ? "" : nil,
        **@attrs
      ) do
        div(class: "flex w-[calc(100vw-2rem)] max-w-4xl h-[85vh] max-h-[680px] bg-card text-card-foreground") do
          sidebar
          content_pane
        end
      end
    end

    private

    def default_path
      item = nav_items.find { |i| i[:key] == @default_section } || nav_items.first
      helpers.public_send(item[:path])
    end

    def sidebar
      div(class: "w-48 flex-shrink-0 border-r border-border flex flex-col bg-muted/30") do
        div(class: "px-4 py-3.5 border-b border-border flex items-center justify-between") do
          h2(class: "text-sm font-semibold text-foreground") { t(".title") }
          form(method: "dialog", class: "contents") do
            button(
              type: "submit",
              class: "p-1 -mr-1 rounded hover:bg-gray-200/70 dark:hover:bg-white/10 text-gray-400 hover:text-gray-600 dark:hover:text-gray-200 transition-colors cursor-pointer border-0 bg-transparent",
              aria_label: t(".close_label")
            ) do
              raw(safe('<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/></svg>'))
            end
          end
        end

        nav(class: "flex-1 overflow-y-auto p-2 space-y-0.5", aria: { label: t(".settings_sections_label") }) do
          nav_items.each { |item| nav_item(item) }
        end
      end
    end

    def nav_item(item)
      active = item[:key] == @default_section
      a(
        href: helpers.public_send(item[:path]),
        aria: { current: active ? "page" : nil },
        data: {
          inbox_settings_modal_target: "navItem",
          section: item[:key],
          turbo_frame: "inbox_settings_panel",
          action: "click->inbox-settings-modal#setActive"
        },
        class: class_names(
          "flex items-center gap-2.5 rounded-lg px-2.5 py-1.5 text-[13px] cursor-pointer transition-colors",
          "text-gray-600 hover:text-gray-900 hover:bg-gray-100/70 dark:text-gray-300 dark:hover:text-white dark:hover:bg-white/5",
          "aria-[current=page]:bg-accent-50 aria-[current=page]:text-accent-700 aria-[current=page]:font-medium",
          "dark:aria-[current=page]:bg-accent-500/10 dark:aria-[current=page]:text-accent-300"
        )
      ) do
        span(class: "w-4 h-4 flex-shrink-0 opacity-70") { raw(safe(nav_icon(item[:icon]))) }
        span { item[:label] }
      end
    end

    def content_pane
      div(class: "flex-1 min-w-0 flex flex-col overflow-hidden bg-background") do
        div(class: "flex-1 min-h-0 overflow-y-auto") do
          panel_frame
        end
        div(class: "px-5 py-3 border-t border-border flex justify-end flex-shrink-0 bg-card") do
          render Campbooks::Button.new(
            variant: :primary,
            size: :sm,
            data: { action: "click->inbox-settings-modal#close" }
          ) { t("shared.actions.done") }
        end
      end
    end

    def panel_frame
      raw(helpers.turbo_frame_tag(
        "inbox_settings_panel",
        loading: "lazy",
        class: "block",
        data: {
          inbox_settings_modal_target: "panel",
          default_src: default_path
        }
      ) do
        helpers.content_tag(:div, t("shared.actions.loading"), class: "flex items-center justify-center py-24 text-sm text-muted-foreground")
      end)
    end

    # Minimal stroke icons keyed by NAV_ITEMS[:icon].
    def nav_icon(name)
      paths = {
        tag:     '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 7h.01M7 3h5c.512 0 1.024.195 1.414.586l7 7a2 2 0 010 2.828l-7 7a2 2 0 01-2.828 0l-7-7A1.994 1.994 0 013 12V7a4 4 0 014-4z"/>',
        doc:     '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 21h10a2 2 0 002-2V9.414a1 1 0 00-.293-.707l-5.414-5.414A1 1 0 0012.586 3H7a2 2 0 00-2 2v14a2 2 0 002 2z"/>',
        users:   '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 20h5v-2a4 4 0 00-3-3.87M9 20H4v-2a4 4 0 013-3.87m6-1.13a4 4 0 10-4-4 4 4 0 004 4z"/>',
        label:   '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 7h.01M3 5a2 2 0 012-2h6l9 9-8 8-9-9V5z"/>',
        pen:     '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z"/>',
        at:      '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M16 12a4 4 0 10-8 0 4 4 0 008 0zm0 0v1.5a2.5 2.5 0 005 0V12a9 9 0 10-9 9m4.5-1.206a8.959 8.959 0 01-4.5 1.206"/>',
        sliders: '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 6V4m0 2a2 2 0 100 4m0-4a2 2 0 110 4m-6 8a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4m6 6v10m6-2a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4"/>',
        filter:  '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 6h18M6 12h12M10 18h4"/>'
      }
      %(<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">#{paths[name]}</svg>)
    end
  end
end
