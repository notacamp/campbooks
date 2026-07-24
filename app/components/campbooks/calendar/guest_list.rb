# frozen_string_literal: true

module Campbooks
  module Calendar
    # Renders a guest list for a calendar event, in either an editable (organizer)
    # or read-only (invitee) state.
    #
    # @param guests [Array<CalendarEvent::Guest>] ordered list of guests
    # @param editable [Boolean] true = organizer view with remove buttons + pill input sync
    class GuestList < Campbooks::Base
      def initialize(guests:, editable: false)
        @guests = guests
        @editable = editable
      end

      def view_template
        return if @guests.empty?

        # min-w-0: this often renders as a grid/flex child; without it the rows'
        # intrinsic width can blow the parent track out past the viewport.
        div(class: "flex flex-col gap-0 min-w-0") do
          if @guests.size > 1
            p(class: "text-xs text-muted-foreground mb-1") { count_summary }
          end

          @guests.each { |guest| render_guest(guest) }
        end
      end

      private

      def count_summary
        parts = [ t(".guests_count", count: @guests.size) ]
        going    = @guests.count { |g| g.rsvp_status == "accepted" }
        maybe    = @guests.count { |g| g.rsvp_status == "tentative" }
        declined = @guests.count { |g| g.rsvp_status == "declined" }
        pending  = @guests.count { |g| g.rsvp_status == "needs_action" }

        parts << t(".going_count",   count: going)    if going    > 0
        parts << t(".maybe_count",   count: maybe)    if maybe    > 0
        parts << t(".declined_count", count: declined) if declined > 0
        parts << t(".pending_count", count: pending)  if pending  > 0

        parts.join(" · ")
      end

      def render_guest(guest)
        div(
          class: "group flex items-center gap-2 py-1",
          data: {
            event_guests_target: "row",
            event_guests_email: guest.email
          }
        ) do
          avatar(guest)
          name_and_email(guest)
          organizer_tag(guest) if show_organizer_tag?(guest)
          status_badge(guest) unless skip_badge?(guest)
          remove_button(guest) if @editable && !guest.self_row
        end
      end

      def avatar(guest)
        initials_text = initials(guest.display_name)
        span(
          class: "w-[26px] h-[26px] rounded-full bg-subtle text-muted-foreground text-[10.5px] font-semibold uppercase flex-none inline-flex items-center justify-center"
        ) { initials_text }
      end

      def name_and_email(guest)
        span(class: "flex-1 min-w-0 flex items-baseline gap-1.5 overflow-hidden") do
          displayed_name = guest.self_row ? t(".you") : guest.display_name
          span(class: "text-[13.5px] font-medium whitespace-nowrap text-foreground") { displayed_name }
          if guest.email != displayed_name
            span(class: "text-xs text-muted-foreground truncate") { guest.email }
          end
        end
      end

      def organizer_tag(guest)
        span(class: "text-[11.5px] text-muted-foreground flex-none") { t(".organizer") }
      end

      def status_badge(guest)
        css, dot_css, label = badge_config(guest.rsvp_status)
        span(class: "inline-flex items-center gap-1.5 rounded-full px-2 py-0.5 text-[11.5px] font-semibold flex-none #{css}") do
          span(class: dot_css)
          plain label
        end
      end

      def remove_button(guest)
        button(
          type: "button",
          class: "opacity-0 group-hover:opacity-100 focus:opacity-100 inline-flex rounded p-0.5 text-muted-foreground hover:text-foreground hover:bg-muted transition-opacity",
          aria_label: t(".remove_guest", name: guest.display_name),
          data: {
            action: "event-guests#removeGuest",
            event_guests_target: "removeBtn",
            email: guest.email
          }
        ) do
          svg(
            class: "w-[13px] h-[13px]",
            viewBox: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2.2"
          ) do |s|
            s.path(
              stroke_linecap: "round",
              stroke_linejoin: "round",
              d: "M6 18L18 6M6 6l12 12"
            )
          end
        end
      end

      # ── Helpers ──────────────────────────────────────────────────────────

      def initials(display_name)
        display_name.to_s.split.first(2).map { |w| w[0]&.upcase }.join
      end

      def show_organizer_tag?(guest)
        guest.organizer || (@editable && guest.self_row)
      end

      # Skip the badge when the organizer has not responded — their implicit RSVP
      # is "accepted" (they created it) and "Pending" would be misleading.
      def skip_badge?(guest)
        guest.organizer && guest.rsvp_status == "needs_action"
      end

      BADGE_CONFIGS = {
        "accepted"     => [ "text-success bg-success/10",    "w-[5px] h-[5px] rounded-full bg-current",                                                    nil ],
        "tentative"    => [ "text-warning bg-warning/10",    "w-[5px] h-[5px] rounded-full bg-current",                                                    nil ],
        "declined"     => [ "text-destructive bg-destructive/10", "w-[5px] h-[5px] rounded-full bg-current",                                               nil ],
        "needs_action" => [ "text-muted-foreground bg-muted", "w-[4px] h-[4px] rounded-full border border-current bg-transparent",                         nil ]
      }.freeze

      def badge_config(rsvp_status)
        css, dot_css, _ = BADGE_CONFIGS.fetch(rsvp_status, BADGE_CONFIGS["needs_action"])
        label = case rsvp_status
        when "accepted"     then t(".going")
        when "tentative"    then t(".maybe")
        when "declined"     then t(".declined")
        else                     t(".pending")
        end
        [ css, dot_css, label ]
      end
    end
  end
end
