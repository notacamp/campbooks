# frozen_string_literal: true

# Previews for the /reminders page row. In-memory Reminder records (ids set so the
# confirm/snooze/dismiss route helpers resolve) stand in for real reminders.
class ReminderRowComponentPreview < ViewComponent::Preview
  # Pending payment-due reminder with an amount and an inline date to confirm.
  def payment_due
    render Campbooks::ReminderRow.new(reminder: reminder)
  end

  # Overdue deadline — the due chip turns red.
  def overdue
    render Campbooks::ReminderRow.new(
      reminder: reminder(reminder_type: "deadline", title: "File Q2 VAT return", due_at: 2.days.ago, amount_cents: nil, description: "Tax authority submission")
    )
  end

  # A timed appointment (not all-day) — shows the clock time.
  def appointment
    render Campbooks::ReminderRow.new(
      reminder: reminder(reminder_type: "appointment", title: "Dentist — Dr. Sousa", due_at: 5.days.from_now.change(hour: 14, min: 30), all_day: false, amount_cents: nil, source_type: "Document")
    )
  end

  # Confirmed reminder linked to a created event.
  def confirmed
    render Campbooks::ReminderRow.new(
      reminder: reminder(status: "confirmed", calendar_event_id: 42)
    )
  end

  private

  def reminder(reminder_type: "payment_due", title: "Pay EDP invoice — January", due_at: 3.days.from_now,
               all_day: true, status: "pending", amount_cents: 18_450, currency: "EUR",
               description: "Monthly electricity bill", source_type: "EmailMessage", calendar_event_id: nil,
               justification: "The invoice states payment is due within 15 days of issue.")
    Reminder.new(
      id: 1, reminder_type: reminder_type, title: title, due_at: due_at, all_day: all_day,
      status: status, amount_cents: amount_cents, currency: currency, description: description,
      source_type: source_type, calendar_event_id: calendar_event_id, justification: justification
    )
  end
end
