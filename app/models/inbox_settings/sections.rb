# frozen_string_literal: true

module InboxSettings
  # The inbox-settings panels — their order and the Turbo-Frame path that renders
  # each one. Settings::InboxController mounts a panel per section under
  # /settings/inbox/:section; the settings overlay lists them (labels and icons
  # live in Settings::Catalog). Every action renders into the
  # `inbox_settings_panel` Turbo Frame.
  module Sections
    ALL = [
      { key: "tags",           path: :inbox_settings_tags_path },
      { key: "tag_groups",     path: :inbox_settings_tag_groups_path },
      { key: "rules",          path: :inbox_settings_rules_path },
      { key: "document_types", path: :inbox_settings_document_types_path },
      { key: "filtering",      path: :inbox_settings_filtering_path },
      { key: "signatures",     path: :inbox_settings_signatures_path },
      { key: "accounts",       path: :inbox_settings_accounts_path },
      { key: "display",        path: :inbox_settings_display_path }
    ].freeze

    module_function

    # The default panel a surface opens to (first in the list).
    def default
      ALL.first
    end
  end
end
