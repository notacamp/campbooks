# frozen_string_literal: true

module Campbooks
  # One row in the personal security/audit log (Settings → Security → activity):
  # an icon tile keyed to the action's category, the human action label
  # (i18n: settings.security.audit_log.actions.<action>), a secondary line (the
  # most useful metadata · IP · device), and a relative timestamp. Mirrors
  # Campbooks::Activity::EventRow.
  class AuditEventRow < Campbooks::Base
    def initialize(event:)
      @event = event
    end

    def view_template
      div(class: "flex items-start gap-3 px-4 py-3") do
        icon_tile
        div(class: "min-w-0 flex-1") do
          div(class: "flex items-baseline justify-between gap-3") do
            p(class: "truncate text-sm font-medium text-foreground") { label }
            time(
              class: "flex-shrink-0 whitespace-nowrap text-xs text-muted-foreground",
              datetime: @event.created_at&.iso8601
            ) { relative_time }
          end
          detail = detail_line
          p(class: "mt-0.5 truncate text-xs text-muted-foreground") { detail } if detail.present?
        end
      end
    end

    private

    def label
      t("settings.security.audit_log.actions.#{@event.action}",
        default: @event.action.to_s.tr("_", " ").humanize)
    end

    def icon_tile
      span(class: "flex size-9 flex-shrink-0 items-center justify-center rounded-lg bg-muted text-muted-foreground") do
        raw(safe(ICONS.fetch(icon_key, ICONS[:default])))
      end
    end

    def icon_key
      case @event.action.to_s
      when "password_changed", /\Amfa_/         then :shield
      when "sign_in", "sign_out", /\Asign_in_method_/ then :login
      when "data_exported"                      then :download
      when "account_deletion_requested"         then :trash
      when "admin_role_changed"                 then :user
      else :default
      end
    end

    def detail_line
      [ metadata_summary, @event.ip_address.presence, device ].compact.reject(&:blank?).join(" · ")
    end

    def metadata_summary
      (@event.metadata || {}).values_at("provider", "role", "method", "nickname").compact.first.presence
    end

    def device
      ua = @event.user_agent.to_s
      ua.present? ? ua.truncate(40) : nil
    end

    def relative_time
      return "" unless @event.created_at

      t("settings.security.audit_log.time_ago", time: helpers.time_ago_in_words(@event.created_at))
    end

    ICONS = {
      shield: '<svg class="w-[18px] h-[18px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3l7 3v5c0 4.5-3 8-7 10-4-2-7-5.5-7-10V6z"/><path d="m9 12 2 2 4-4"/></svg>',
      login: '<svg class="w-[18px] h-[18px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M15 3h4a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2h-4M10 17l5-5-5-5M15 12H3"/></svg>',
      download: '<svg class="w-[18px] h-[18px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3v12m0 0 4-4m-4 4-4-4M5 21h14"/></svg>',
      trash: '<svg class="w-[18px] h-[18px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M3 6h18M8 6V4h8v2m-9 0v14a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1V6"/></svg>',
      user: '<svg class="w-[18px] h-[18px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="8" r="4"/><path d="M4 21a8 8 0 0 1 16 0"/></svg>',
      default: '<svg class="w-[18px] h-[18px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3.2"/><circle cx="12" cy="12" r="9"/></svg>'
    }.freeze
  end
end
