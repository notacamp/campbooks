# frozen_string_literal: true

class CalendarGuestListPreview < Lookbook::Preview
  # Editable list: all four RSVP statuses, an organizer row (self), and extra guests.
  # Mimics the organizer's view of their own event (specimen 2 in the design prototype).
  def editable
    guests = [
      CalendarEvent::Guest.new(
        email: "gui@example.com",
        name: "Guilherme Andrade",
        rsvp_status: "accepted",
        self_row: true,
        organizer: true
      ),
      CalendarEvent::Guest.new(
        email: "maya@example.com",
        name: "Maya Chen",
        rsvp_status: "accepted",
        self_row: false,
        organizer: false
      ),
      CalendarEvent::Guest.new(
        email: "rita@example.com",
        name: "Rita Alves",
        rsvp_status: "declined",
        self_row: false,
        organizer: false
      ),
      CalendarEvent::Guest.new(
        email: "rui.marques@example.com",
        name: "Rui Marques",
        rsvp_status: "needs_action",
        self_row: false,
        organizer: false
      ),
      CalendarEvent::Guest.new(
        email: "sam@example.com",
        name: "Sam Ortiz",
        rsvp_status: "tentative",
        self_row: false,
        organizer: false
      )
    ]
    render(Campbooks::Calendar::GuestList.new(guests: guests, editable: true))
  end

  # Read-only list: the current user is an invitee viewing an event organized by
  # someone else (specimen 3 in the design prototype).
  def read_only
    guests = [
      CalendarEvent::Guest.new(
        email: "jonas@example.com",
        name: "Jonas Ferreira",
        rsvp_status: "accepted",
        self_row: false,
        organizer: true
      ),
      CalendarEvent::Guest.new(
        email: "gui@example.com",
        name: "Guilherme Andrade",
        rsvp_status: "accepted",
        self_row: true,
        organizer: false
      ),
      CalendarEvent::Guest.new(
        email: "maya@example.com",
        name: "Maya Chen",
        rsvp_status: "needs_action",
        self_row: false,
        organizer: false
      )
    ]
    render(Campbooks::Calendar::GuestList.new(guests: guests, editable: false))
  end
end
