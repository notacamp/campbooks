# frozen_string_literal: true

module Api
  module App
    module Time
      # Wraps the full Time surface read model — the merged agenda, undated asks,
      # Scout day note, and focus-slot suggestions — into one JSON envelope. This is
      # what `GET /api/app/time` returns.
      class AgendaSerializer
        def initialize(items:, undated:, day_note:, suggestions:)
          @items       = items
          @undated     = undated
          @day_note    = day_note
          @suggestions = suggestions
        end

        def as_json
          {
            items:       @items.map { |item| AgendaItemSerializer.new(item).as_json },
            undated:     @undated.map { |item| AgendaItemSerializer.new(item).as_json },
            day_note:    serialize_day_note(@day_note),
            suggestions: @suggestions.map { |s| serialize_suggestion(s) }
          }
        end

        private

        def serialize_day_note(dn)
          return nil unless dn

          {
            date:                dn.date&.iso8601,
            meetings_count:      dn.meetings_count,
            deadlines_count:     dn.deadlines_count,
            first_deadline_title: dn.first_deadline_title,
            undated_count:       dn.undated_count,
            focus:               serialize_day_note_focus(dn.focus),
            late_obligation:     serialize_late_obligation(dn.late_obligation),
            prep_meeting:        serialize_prep_meeting(dn.prep_meeting)
          }
        end

        def serialize_day_note_focus(f)
          return nil unless f

          {
            title:            f.title,
            subject:          f.subject,
            at:               f.at&.iso8601,
            duration_minutes: f.duration_minutes
          }
        end

        def serialize_late_obligation(lo)
          return nil unless lo

          { name: lo.name, days_overdue: lo.days_overdue }
        end

        def serialize_prep_meeting(pm)
          return nil unless pm

          {
            title:      pm.title,
            at:         pm.at&.iso8601,
            first_name: pm.first_name,
            why:        pm.why
          }
        end

        def serialize_suggestion(s)
          {
            starts_at:        s.respond_to?(:starts_at) ? s.starts_at&.iso8601 : s[:starts_at]&.iso8601,
            ends_at:          s.respond_to?(:ends_at)   ? s.ends_at&.iso8601   : s[:ends_at]&.iso8601,
            duration_minutes: s.respond_to?(:duration_minutes) ? s.duration_minutes : s[:duration_minutes]
          }
        end
      end
    end
  end
end
