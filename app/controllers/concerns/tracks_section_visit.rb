# Records that the acting user has "seen" a primary-nav section, clearing its
# attention dot (Navigation::Attention). Used as a class macro in the
# controllers that render a section's landing view:
#
#   class CalendarController < ApplicationController
#     tracks_section_visit :calendar, only: :index
#   end
#
# Runs as a before_action so the page you just landed on already renders with its
# own dot cleared. No-ops for unauthenticated requests (Current.user is nil).
module TracksSectionVisit
  extend ActiveSupport::Concern

  class_methods do
    def tracks_section_visit(section, **options)
      before_action(**options) { Current.user&.mark_section_seen!(section) }
    end
  end
end
