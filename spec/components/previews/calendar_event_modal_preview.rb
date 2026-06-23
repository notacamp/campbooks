# frozen_string_literal: true

class CalendarEventModalPreview < Lookbook::Preview
  # The modal shell, open on its loading state. In the app the Turbo Frame
  # lazy-loads the new/edit event form (app/views/calendar_events/{new,edit})
  # into it; here the frame has no src, so the loading placeholder shows.
  def default
    render(Campbooks::Calendar::EventModal.new(open: true))
  end
end
