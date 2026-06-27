# frozen_string_literal: true

class ScheduledEmail < ApplicationRecord
  belongs_to :workspace
  belongs_to :email_account
  belongs_to :created_by, class_name: "User"

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

  def display_time
    next_occurrence_at || scheduled_at
  end

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
