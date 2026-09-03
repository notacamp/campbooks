# frozen_string_literal: true

# Turns a proposed (or moved) focus block into a kept commitment. With a writable
# calendar it creates a real CalendarEvent there — the same local-event →
# outbound-sync path Reminders::Confirm uses (Calendars::WritableTarget picks the
# calendar, EventWriteJob pushes it), titled "Focus: …" and linked to the source
# email so the row links back. With no writable calendar the block is simply kept
# locally (still shown on the agenda). Idempotent — a re-Keep is a no-op.
class Time::FocusKeeper
  Result = Data.define(:success, :focus_block, :calendar_event, :error) do
    def success? = success
    def calendar? = !calendar_event.nil?
  end

  def self.call(focus_block, user: Current.user)
    new(focus_block, user).call
  end

  def initialize(focus_block, user)
    @block = focus_block
    @user = user
  end

  def call
    return already_kept if @block.kept? && @block.calendar_event_id

    calendar = Calendars::WritableTarget.for(user: @user, source_email_account: source_email_account)
    unless calendar
      @block.update!(status: :kept)
      return Result.new(success: true, focus_block: @block, calendar_event: nil, error: nil)
    end

    event = calendar.calendar_events.create!(
      provider_event_id:    "local-#{SecureRandom.uuid}",
      title:                @block.title,
      start_at:             @block.start_at,
      end_at:               @block.end_at,
      all_day:              false,
      status:               :confirmed,
      outbound_pending:     true,
      source_email_message: source_email
    )

    Calendars::EventWriteJob.perform_later(event.id, "create")
    @block.update!(status: :kept, calendar_event: event)
    Result.new(success: true, focus_block: @block, calendar_event: event, error: nil)
  rescue ActiveRecord::RecordInvalid => e
    Result.new(success: false, focus_block: @block, calendar_event: nil, error: e.message)
  end

  private

  def already_kept
    Result.new(success: true, focus_block: @block, calendar_event: @block.calendar_event, error: nil)
  end

  # CalendarEvent links back to an email source only; a document-sourced deadline
  # keeps its link on the reminder.
  def source_email
    @block.reminder&.source_email
  end

  def source_email_account
    reminder = @block.reminder
    return nil unless reminder

    case reminder.source
    when EmailMessage then reminder.source.email_account
    when Document     then reminder.source.email_messages.first&.email_account
    end
  end
end
