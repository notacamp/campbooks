# frozen_string_literal: true

module Digests
  module Sources
    # Gathers upcoming Reminders in the lookahead window. Mirrors the filters in
    # NeedsAttentionDigestMailJob (pending, confidence >= 0.6, not Task-sourced).
    # Task-sourced reminders are excluded to avoid double-listing with the tasks
    # source when both are configured.
    class Reminders < Base
      MIN_CONFIDENCE = 0.6

      def self.direction = :lookahead

      def items(period)
        # The generator already applied window_days when computing the period.
        Reminder.accessible_to(user)
                .pending
                .where(confidence: MIN_CONFIDENCE..)
                .where(due_at: period.begin..period.end)
                .where.not(source_type: "Task")
                .order(:due_at)
                .limit(MAX_ITEMS)
                .map do |reminder|
                  Digests::Item.new(
                    source_type: "reminder",
                    source_id:   reminder.id,
                    title:       reminder.title,
                    subtitle:    reminder_subtitle(reminder),
                    summary:     nil,
                    timestamp:   reminder.due_at&.iso8601
                  )
                end
      end

      private

      def reminder_subtitle(reminder)
        date_label = I18n.l(reminder.due_at, format: :short)
        type_label = I18n.t(
          "activerecord.attributes.reminder.reminder_types.#{reminder.reminder_type}",
          default: reminder.reminder_type.to_s.humanize
        )
        "#{date_label} · #{type_label}"
      end
    end
  end
end
