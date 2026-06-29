# frozen_string_literal: true

# A one-time or recurring email queued to send at a future time. The compose
# area ("Schedule" button) and the dedicated /scheduled_emails surface both
# create these; ScheduledEmailSendJob (every minute) dispatches the due ones via
# Emails::Sender and advances recurring items to their next occurrence.
class ScheduledEmail < ApplicationRecord
  belongs_to :workspace
  belongs_to :email_account
  belongs_to :created_by, class_name: "User"
  belongs_to :email_template, optional: true

  enum :status, { pending: 0, sent: 1, cancelled: 2, failed: 3 }, default: :pending

  validates :to_address, presence: true
  validates :subject, presence: true
  validates :body, presence: true
  validates :scheduled_at, presence: true

  scope :accessible_to, ->(user) {
    return none unless user
    where(workspace_id: user.workspace_id)
  }

  scope :due, -> { pending.where("scheduled_at <= ?", Time.current) }

  scope :in_range, ->(from, to) {
    pending.where(
      "COALESCE(next_occurrence_at, scheduled_at) >= ? AND COALESCE(next_occurrence_at, scheduled_at) < ?",
      from, to
    )
  }

  def recurring?
    rrule.present?
  end

  # Maps the constrained RRULE values the UI offers back to a translation key
  # (scheduled_emails.recurrence.*), so views never surface a raw RRULE string.
  def recurrence_key
    case rrule
    when "FREQ=DAILY" then :daily
    when "FREQ=WEEKLY" then :weekly
    when "FREQ=WEEKLY;INTERVAL=2" then :biweekly
    when "FREQ=MONTHLY" then :monthly
    else :custom
    end
  end

  # The time this item next fires: the computed next occurrence for recurring
  # items, otherwise the one-time scheduled_at.
  def display_time
    next_occurrence_at || scheduled_at
  end

  # Subject/body rendered through Liquid against the stored template_context, so a
  # templated (and especially recurring) send re-resolves variables like
  # {{ date }} fresh on every occurrence. Plain sends carry an empty context, so
  # this is a no-op for them.
  def rendered_subject
    render_liquid(subject)
  end

  def rendered_body
    render_liquid(body)
  end

  private

  def render_liquid(template)
    Workflows::LiquidRenderer.render(template, template_context)
  rescue Workflows::LiquidRenderer::Error => e
    Rails.logger.warn("[ScheduledEmail##{id}] Liquid render error: #{e.message}")
    template
  end
end
