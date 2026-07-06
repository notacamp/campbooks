# frozen_string_literal: true

module InboxSettings
  # Single source of truth for the inbox-settings panels — their order, the
  # Turbo-Frame path that renders each one, and its nav icon. Read by BOTH
  # surfaces that expose these settings so the two can never drift:
  #
  #   * Campbooks::InboxSettingsModal — the gear-icon dialog on the inbox
  #   * Settings::InboxController      — the Settings -> Inbox dashboard page
  #
  # Adding or reordering a panel here updates both surfaces at once. Labels live
  # in the shared i18n namespace `components.inbox_settings_modal.nav.*` (see
  # `.label_key`), which both surfaces resolve.
  module Sections
    # Order matches the approved modal layout: management sections first, the
    # per-device Display preferences last. `:path` is a named route helper; every
    # action renders into the `inbox_settings_panel` Turbo Frame.
    ALL = [
      { key: "tags",           path: :inbox_settings_tags_path,           icon: :tag },
      { key: "document_types", path: :inbox_settings_document_types_path,  icon: :doc },
      { key: "filtering",      path: :inbox_settings_filtering_path,       icon: :filter },
      { key: "signatures",     path: :inbox_settings_signatures_path,      icon: :pen },
      { key: "accounts",       path: :inbox_settings_accounts_path,        icon: :at },
      { key: "display",        path: :inbox_settings_display_path,         icon: :sliders }
    ].freeze

    # Stroke-icon `d` path data keyed by `:icon`. Kept as the bare `d` attribute so
    # both consumers can render it: the settings sidebar drops it straight into its
    # own <svg><path d=…> template (NavigationHelper), and `.icon_svg` wraps it for
    # the Phlex modal nav.
    ICON_PATHS = {
      tag:     "M7 7h.01M7 3h5c.512 0 1.024.195 1.414.586l7 7a2 2 0 010 2.828l-7 7a2 2 0 01-2.828 0l-7-7A1.994 1.994 0 013 12V7a4 4 0 014-4z",
      doc:     "M7 21h10a2 2 0 002-2V9.414a1 1 0 00-.293-.707l-5.414-5.414A1 1 0 0012.586 3H7a2 2 0 00-2 2v14a2 2 0 002 2z",
      filter:  "M3 6h18M6 12h12M10 18h4",
      stack:   "M20 7l-8-4-8 4m16 0l-8 4m8-4v10l-8 4m0-10L4 7m8 4v10M4 7v10l8 4",
      pen:     "M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z",
      at:      "M16 12a4 4 0 10-8 0 4 4 0 008 0zm0 0v1.5a2.5 2.5 0 005 0V12a9 9 0 10-9 9m4.5-1.206a8.959 8.959 0 01-4.5 1.206",
      sliders: "M12 6V4m0 2a2 2 0 100 4m0-4a2 2 0 110 4m-6 8a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4m6 6v10m6-2a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4"
    }.freeze

    module_function

    # The default panel a surface opens to (first in the list).
    def default
      ALL.first
    end

    # i18n key for a section's nav label. Shared across the modal + settings pages.
    def label_key(key)
      "components.inbox_settings_modal.nav.#{key}"
    end

    # A section's nav icon as an inline <svg> string (for the Phlex modal nav).
    def icon_svg(name, css: "w-4 h-4")
      %(<svg class="#{css}" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="#{ICON_PATHS[name]}"/></svg>)
    end
  end
end
