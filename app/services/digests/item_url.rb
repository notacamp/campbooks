# frozen_string_literal: true

module Digests
  # Server-side URL resolution for digest issue items. Consistent with the URL
  # choices in DigestMailer#needs_attention (tasks_url, reminders_url, etc.) and
  # the calendar/document show-page patterns.
  module ItemUrl
    def self.for(source_type, source_id)
      host_options = Rails.application.config.action_mailer.default_url_options || {}

      case source_type.to_s
      when "email"
        Rails.application.routes.url_helpers.email_message_url(source_id, **host_options)
      when "calendar_event"
        Rails.application.routes.url_helpers.calendar_url(**host_options)
      when "task"
        Rails.application.routes.url_helpers.tasks_url(**host_options)
      when "reminder"
        Rails.application.routes.url_helpers.reminders_url(**host_options)
      when "document"
        Rails.application.routes.url_helpers.document_url(source_id, **host_options)
      else
        Rails.application.routes.url_helpers.root_url(**host_options)
      end
    rescue => e
      Rails.logger.warn("[Digests::ItemUrl] Could not build URL for #{source_type}/#{source_id}: #{e.message}")
      "/"
    end
  end
end
