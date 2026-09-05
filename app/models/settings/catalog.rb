# frozen_string_literal: true

module Settings
  # Single source of truth for the settings information architecture.
  # Consumed by: Campbooks::SettingsOverlay (nav), CommandPaletteCatalog#settings, specs.
  # Replaces NavigationHelper#settings_nav_groups and #inbox_settings_nav_group.
  module Catalog
    Group = Struct.new(:key, :items, :ember, keyword_init: true)
    Item  = Struct.new(:key, :group, :icon, :route, :active_keys, :visible, keyword_init: true)

    Context = Struct.new(:user, :native, keyword_init: true) do
      def native? = native
    end

    # Icon path data keyed by symbol. 24x24, stroke-based unless noted.
    ICON_PATHS = {
      user:            '<circle cx="12" cy="8" r="4"/><path d="M4 21a8 8 0 0 1 16 0"/>',
      lock:            '<rect x="4" y="11" width="16" height="10" rx="2"/><path d="M8 11V7a4 4 0 0 1 8 0v4"/>',
      bell:            '<path d="M6 8a6 6 0 0 1 12 0c0 7 3 7 3 9H3c0-2 3-2 3-9"/><path d="M10 21a2 2 0 0 0 4 0"/>',
      spark:           '<path d="M12 5l1.7 5.6L19.5 12l-5.8 1.4L12 19l-1.7-5.6L4.5 12l5.8-1.4z"/>',
      cpu:             '<rect x="5" y="5" width="14" height="14" rx="2"/><rect x="9" y="9" width="6" height="6"/><path d="M9 2v3M15 2v3M9 19v3M15 19v3M2 9h3M2 15h3M19 9h3M19 15h3"/>',
      chat:            '<path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/>',
      template:        '<rect x="3" y="3" width="18" height="7" rx="1"/><rect x="3" y="14" width="9" height="7" rx="1"/><rect x="16" y="14" width="5" height="7" rx="1"/>',
      "mail-template": '<rect x="3" y="5" width="18" height="14" rx="2"/><path d="m3 7 9 6 9-6"/>',
      zap:             '<path d="M13 2L4 14h7l-1 8 9-12h-7z"/>',
      tag:             '<path d="M20.6 13.4 13.4 20.6a2 2 0 0 1-2.8 0L3 13V3h10l7.6 7.6a2 2 0 0 1 0 2.8z"/><path d="M7 7h.01"/>',
      layers:          '<path d="m12 3 9 4.5-9 4.5-9-4.5z"/><path d="m3 12 9 4.5 9-4.5"/><path d="m3 16.5 9 4.5 9-4.5"/>',
      filter:          '<path d="M3 6h18M6 12h12M10 18h4"/>',
      pen:             '<path d="M12 20h9"/><path d="M16.5 3.5a2.12 2.12 0 0 1 3 3L7 19l-4 1 1-4z"/>',
      eye:             '<path d="M2 12s4-7 10-7 10 7 10 7-4 7-10 7S2 12 2 12z"/><circle cx="12" cy="12" r="3"/>',
      file:            '<path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><path d="M14 2v6h6"/>',
      branch:          '<path d="M6 3v12"/><circle cx="18" cy="6" r="3"/><circle cx="6" cy="18" r="3"/><path d="M18 9a9 9 0 0 1-9 9"/>',
      at:              '<circle cx="12" cy="12" r="4"/><path d="M16 12v1.5a2.5 2.5 0 0 0 5 0V12a9 9 0 1 0-5.5 8.3"/>',
      calendar:        '<rect x="3" y="4.5" width="18" height="16.5" rx="2"/><path d="M3 9.5h18M8 3v4M16 3v4"/>',
      drive:           '<path d="M22 12H2M5.5 5.1 2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.5-6.9A2 2 0 0 0 16.7 4H7.3a2 2 0 0 0-1.8 1.1z"/><path d="M6 16h.01M10 16h.01"/>',
      notion:          '<rect x="4" y="4" width="16" height="16" rx="2"/><path d="M8 16V8l8 8V8"/>',
      cloud:           '<path d="M17.5 19a4.5 4.5 0 1 0-.9-8.9A6 6 0 0 0 5 12a4 4 0 0 0 1 7.9z"/>',
      plug:            '<path d="M12 22v-5M9 8V2M15 8V2M6 8h12l-1 5a5 5 0 0 1-10 0z"/>',
      key:             '<circle cx="7.5" cy="15.5" r="3.5"/><path d="M10 13l9-9M15 5l3 3"/>',
      building:        '<rect x="4" y="3" width="16" height="18" rx="1"/><path d="M8 7h2M14 7h2M8 11h2M14 11h2M8 15h2M14 15h2M10 21v-4h4v4"/>',
      users:           '<path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/><path d="M22 21v-2a4 4 0 0 0-3-3.87"/><path d="M16 3.13a4 4 0 0 1 0 7.75"/>',
      grid:            '<rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/><rect x="14" y="14" width="7" height="7" rx="1"/>',
      "shield-check":  '<path d="M12 3l8 3v6c0 5-3.5 8.5-8 9.5C7.5 20.5 4 17 4 12V6z"/><path d="m9 12 2 2 4-4"/>',
      card:            '<rect x="2" y="6" width="20" height="12" rx="2"/><path d="M2 10h20M6 15h4"/>',
      pulse:           '<path d="M3 12h4l3-8 4 16 3-8h4"/>',
      sliders:         '<path d="M4 7h9M17 7h3M4 17h3M11 17h9"/><circle cx="15" cy="7" r="2"/><circle cx="9" cy="17" r="2"/>',
      search:          '<circle cx="11" cy="11" r="7"/><path d="m21 21-4.3-4.3"/>',
      x:               '<path d="M6 6l12 12M18 6 6 18"/>',
      "chevron-right": '<path d="m9 18 6-6-6-6"/>',
      "chevron-left":  '<path d="m15 18-6-6 6-6"/>',
      keyboard:        '<rect x="3" y="6" width="18" height="12" rx="2"/><path d="M7 10h.01M11 10h.01M15 10h.01M7 14h10"/>',
      contrast:        '<circle cx="12" cy="12" r="9"/><path d="M12 3a9 9 0 0 1 0 18z" fill="currentColor" stroke="none"/>',
      sun:             '<circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/>',
      moon:            '<path d="M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8z"/>',
      monitor:         '<rect x="3" y="5" width="18" height="12" rx="2"/><path d="M8 21h8M12 17v4"/>',
      shield:          '<path d="M12 3l8 3v6c0 5-3.5 8.5-8 9.5C7.5 20.5 4 17 4 12V6z"/>',
      logout:          '<path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4M16 17l5-5-5-5M21 12H9"/>'
    }.freeze

    # Item -> icon mapping
    ITEM_ICONS = {
      account:            :user,
      security:           :lock,
      notifications:      :bell,
      memory:             :spark,
      ai:                 :cpu,
      guidance:           :chat,
      document_templates: :template,
      email_templates:    :"mail-template",
      automations:        :zap,
      rules:              :zap,
      tags:               :tag,
      streams:            :layers,
      filtering:          :filter,
      signatures:         :pen,
      display:            :eye,
      document_types:     :file,
      pipelines:          :branch,
      mail_accounts:      :at,
      calendars:          :calendar,
      google_drive:       :drive,
      notion:             :notion,
      zoho_drive:         :cloud,
      connections:        :plug,
      api_access:         :key,
      general:            :building,
      members:            :users,
      modules:            :grid,
      data_privacy:       :"shield-check",
      plan:               :card,
      system_health:      :pulse
    }.freeze

    GROUPS_DEFINITION = [
      {
        key: :you,
        items: [
          { key: :account,       route: :settings_account_path,       active_keys: %w[account] },
          { key: :security,      route: :settings_security_path,       active_keys: %w[security totp passkeys recovery_codes email_otp sign_in_methods audit_log] },
          { key: :notifications, route: :settings_notifications_path,  active_keys: %w[notifications] }
        ]
      },
      {
        key: :scout,
        ember: true,
        items: [
          { key: :memory,             route: :settings_memory_path,             active_keys: %w[memory] },
          { key: :ai,                 route: :settings_ai_path,                 active_keys: %w[ai ai_adapters] },
          { key: :guidance,           route: :settings_ai_prompts_path,         active_keys: %w[ai_prompts] },
          { key: :document_templates, route: :settings_document_templates_path, active_keys: %w[document_templates],
            visible: ->(_ctx) { Features.document_templates? } },
          { key: :email_templates,    route: :settings_email_templates_path,    active_keys: %w[email_templates],
            visible: ->(_ctx) { Features.email_templates? } },
          { key: :automations,        route: :workflows_path,                   active_keys: [],
            visible: ->(_ctx) { Features.workflows? } }
        ]
      },
      {
        key: :inbox,
        items: [
          { key: :rules,      route: ->(_vc) { Rails.application.routes.url_helpers.settings_inbox_section_path("rules") },      active_keys: %w[inbox_rules] },
          { key: :tags,       route: ->(_vc) { Rails.application.routes.url_helpers.settings_inbox_section_path("tags") },       active_keys: %w[inbox_tags] },
          { key: :streams,    route: ->(_vc) { Rails.application.routes.url_helpers.settings_inbox_section_path("tag_groups") }, active_keys: %w[inbox_tag_groups] },
          { key: :filtering,  route: ->(_vc) { Rails.application.routes.url_helpers.settings_inbox_section_path("filtering") },  active_keys: %w[inbox_filtering] },
          { key: :signatures, route: ->(_vc) { Rails.application.routes.url_helpers.settings_inbox_section_path("signatures") }, active_keys: %w[inbox_signatures] },
          { key: :display,    route: ->(_vc) { Rails.application.routes.url_helpers.settings_inbox_section_path("display") },    active_keys: %w[inbox_display] }
        ]
      },
      {
        key: :paper,
        items: [
          { key: :document_types, route: ->(_vc) { Rails.application.routes.url_helpers.settings_inbox_section_path("document_types") }, active_keys: %w[inbox_document_types] },
          { key: :pipelines,      route: :settings_pipelines_path, active_keys: %w[pipelines] }
        ]
      },
      {
        key: :connections,
        items: [
          { key: :mail_accounts, route: ->(_vc) { Rails.application.routes.url_helpers.settings_inbox_section_path("accounts") }, active_keys: %w[inbox_accounts] },
          { key: :calendars,     route: :settings_integrations_calendars_path,    active_keys: %w[calendars] },
          { key: :google_drive,  route: :settings_integrations_google_drive_path, active_keys: %w[google_drive google_drive_configs] },
          { key: :notion,        route: :settings_integrations_notion_path,       active_keys: %w[notion] },
          { key: :zoho_drive,    route: :settings_integrations_zoho_drive_path,   active_keys: %w[zoho_drive] },
          { key: :connections,   route: :settings_integrations_connections_path,  active_keys: %w[connections] },
          { key: :api_access,    route: :settings_api_clients_path,               active_keys: %w[api_clients],
            visible: ->(ctx) { !ctx.native? } }
        ]
      },
      {
        key: :workspace,
        items: [
          { key: :general,       route: :settings_root_path,         active_keys: %w[general] },
          { key: :members,       route: :settings_members_path,      active_keys: %w[members invitations] },
          { key: :modules,       route: :settings_setup_template_path, active_keys: %w[setup_template] },
          { key: :data_privacy,  route: :settings_data_privacy_path, active_keys: %w[data_privacy] },
          { key: :plan,          route: :settings_plan_path,         active_keys: %w[plan] },
          { key: :system_health, route: :settings_system_health_path, active_keys: %w[system_health],
            visible: ->(ctx) { ctx.user&.admin? } }
        ]
      }
    ].freeze

    module_function

    def groups(context)
      h = Rails.application.routes.url_helpers
      GROUPS_DEFINITION.filter_map do |gdef|
        resolved = gdef[:items].filter_map do |idef|
          next if idef[:visible] && !idef[:visible].call(context)

          path = if idef[:route].is_a?(Symbol)
                   h.public_send(idef[:route])
                 else
                   idef[:route].call(h)
                 end
          {
            key:         idef[:key],
            label:       I18n.t("settings.catalog.items.#{idef[:key]}"),
            icon:        ITEM_ICONS[idef[:key]],
            path:        path,
            active_keys: idef[:active_keys]
          }
        end
        next if resolved.empty?

        Group.new(
          key:   gdef[:key],
          items: resolved,
          ember: gdef[:ember] || false
        )
      end
    end

    def item_for_section(section, context)
      groups(context).each do |group|
        item = group.items.find { |i| i[:active_keys].include?(section.to_s) }
        return item if item
      end
      nil
    end

    def default_path
      Rails.application.routes.url_helpers.settings_account_path
    end

    def icon_svg(name, css: "w-4 h-4")
      path = ICON_PATHS[name.to_sym] || ""
      fill_attrs = name.to_sym == :spark ? 'fill="currentColor"' : 'fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"'
      %(<svg class="#{css}" viewBox="0 0 24 24" #{fill_attrs} aria-hidden="true">#{path}</svg>)
    end
  end
end
