class Workflow < ApplicationRecord
  belongs_to :workspace
  # The user who built the workflow; email_action steps run as this user so
  # EmailActions' per-user permission gates apply. Optional for workflows
  # created before the column existed (their email_action steps fail closed).
  belongs_to :created_by, class_name: "User", optional: true

  has_many :steps, -> { order(:position) }, class_name: "WorkflowStep", dependent: :destroy
  has_many :executions, -> { order(created_at: :desc) }, class_name: "WorkflowExecution", dependent: :destroy

  accepts_nested_attributes_for :steps, allow_destroy: true

  validates :name, presence: true
  validates :trigger_type, presence: true

  scope :enabled, -> { where(enabled: true) }

  # Triggers decide *what* kicks a workflow off:
  #   email_received — an inbound email finished processing (internal trigger)
  #   webhook        — an external service POSTed to this workflow's unique URL
  #   event          — a domain Event was published; trigger_config["event_name"]
  #                    selects which one (exact "document.approved" or prefix
  #                    wildcard "document.*"). See Events::Registry for the catalog.
  TRIGGER_TYPES = %w[email_received webhook event].freeze

  validates :trigger_type, inclusion: { in: TRIGGER_TYPES }
  validates :webhook_token, uniqueness: true, allow_nil: true

  before_validation :ensure_webhook_token

  def email_trigger?
    trigger_type == "email_received"
  end

  def webhook?
    trigger_type == "webhook"
  end

  def event_trigger?
    trigger_type == "event"
  end

  # Rotate the inbound URL — old callers stop working immediately.
  def regenerate_webhook_token!
    update!(webhook_token: self.class.generate_webhook_token)
  end

  def self.generate_webhook_token
    SecureRandom.urlsafe_base64(24)
  end

  private

  # A webhook workflow is useless without a token; mint one the moment the
  # trigger becomes a webhook so the URL is ready to copy on first save.
  def ensure_webhook_token
    self.webhook_token = self.class.generate_webhook_token if webhook? && webhook_token.blank?
  end
end
