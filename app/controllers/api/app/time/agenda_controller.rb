# frozen_string_literal: true

module Api
  module App
    module Time
      # GET /api/app/time — the merged time-surface read: calendar events + deadlines
      # + asks + focus blocks in one chronological agenda, plus undated asks and the
      # Scout day note. Mirrors TimeController#index but returns JSON instead of HTML.
      #
      # Query params:
      #   date       (optional) ISO-8601 date; defaults to today in the user's zone
      #   view       (optional) agenda | week | month; default agenda (30-day window)
      class AgendaController < Api::App::BaseController
        WINDOW = {
          "agenda" => 30.days,
          "week"   => 7.days,
          "month"  => 31.days
        }.freeze

        def show
          zone  = current_user.effective_time_zone
          today = ::Time.current.in_time_zone(zone).to_date
          date  = parse_date(params[:date]) || today
          view  = params[:view].to_s.presence_in(WINDOW.keys) || "agenda"

          from = zone.local(date.year, date.month, date.day).beginning_of_day
          to   = from + WINDOW[view]

          items    = ::Time::Agenda.for(current_user, from: from, to: to)
          undated  = ::Time::Agenda.undated_for(current_user)
          day_note = ::Time::DayNote.for(current_user, date: date)

          suggestions = begin
            ::Time::FocusProposer.for(current_user, today: date)
          rescue StandardError
            []
          end

          serialized = Api::App::Time::AgendaSerializer.new(
            items:       items,
            undated:     undated,
            day_note:    day_note,
            suggestions: suggestions
          ).as_json

          render_data(serialized)
        end

        private

        def parse_date(value)
          return nil if value.blank?

          ::Date.parse(value.to_s)
        rescue Date::Error, ArgumentError
          nil
        end
      end
    end
  end
end
