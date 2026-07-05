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

    # The panels + their order come from the shared catalog (InboxSettings::
    # Sections) so this modal and the Settings -> Inbox page never drift. Labels
    # are resolved here so t() runs at render time (not class-load).
    def nav_items
      InboxSettings::Sections::ALL.map do |section|
        section.merge(label: t(InboxSettings::Sections.label_key(section[:key])))
      end
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
        div(class: "flex flex-col sm:flex-row w-[calc(100vw-2rem)] max-w-4xl h-[85vh] max-h-[680px] bg-card text-card-foreground") do
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
      div(class: "w-full sm:w-48 flex-shrink-0 border-b sm:border-b-0 sm:border-r border-border flex flex-col bg-muted/30") do
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

        nav(class: "flex gap-1 overflow-x-auto scrollbar-none sm:flex-1 sm:flex-col sm:gap-0 sm:space-y-0.5 sm:overflow-y-auto p-2", aria: { label: t(".settings_sections_label") }) do
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
          "flex shrink-0 whitespace-nowrap items-center gap-2.5 rounded-lg px-2.5 py-1.5 text-[13px] cursor-pointer transition-colors",
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

    # Nav icons come from the shared section catalog (InboxSettings::Sections).
    def nav_icon(name)
      InboxSettings::Sections.icon_svg(name)
    end
  end
end
