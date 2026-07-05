# frozen_string_literal: true

module Digests
  module Sources
    # Gathers upcoming CalendarEvents in the lookahead window. Mirrors the
    # CalendarController agenda composition: concrete events + series masters
    # expanded via Calendars::OccurrenceExpander (so the digest matches what the
    # user sees on the calendar page, including locally-managed recurring events).
    class Calendar < Base
      def self.direction = :lookahead

      def items(period)
        # The generator already applied window_days when computing the period.
        from = period.begin
        to   = period.end

        base     = CalendarEvent.accessible_to(user).visible
        concrete = base.concrete.in_range(from, to).order(:start_at)
        masters  = base.series_masters

        events = Calendars::OccurrenceExpander.new(
          concrete: concrete, masters: masters, from: from, to: to
        ).events

        events.first(MAX_ITEMS).map do |evt|
          Digests::Item.new(
            source_type: "calendar_event",
            source_id:   evt.id,
            title:       evt.title.to_s.presence || I18n.t("digests.sections.calendar"),
            subtitle:    event_subtitle(evt),
            summary:     truncate(evt.description.to_s),
            timestamp:   evt.start_at&.iso8601
          )
        end
      end

      private

      def event_subtitle(evt)
        parts = [ time_range_label(evt) ]
        parts << evt.location if evt.location.present?
        parts.join(" · ")
      end

      def time_range_label(evt)
        if evt.all_day
          I18n.l(evt.start_at.to_date, format: :short)
        else
          start_str = I18n.l(evt.start_at, format: :short)
          end_str   = I18n.l(evt.end_at, format: :short) if evt.end_at && evt.end_at != evt.start_at
          [ start_str, end_str ].compact.join(" – ")
        end
      end
    end
  end
end
