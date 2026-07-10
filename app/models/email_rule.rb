# frozen_string_literal: true

# A workspace-scoped, user-defined rule that is evaluated against every newly
# ingested email (and can be run retroactively).  When an email matches the
# rule's criteria (AND across keys, OR within a key's array) the rule applies
# its configured actions.
#
# Criteria shape — store only present keys (blank values are stripped):
#   from:             [String] — any-of ILIKE against from_address.
#                     A value starting with "@" matches any address at that domain.
#   to:               [String] — any-of ILIKE against to_address OR cc_address.
#   subject:          [String] — any-of ILIKE contains against subject.
#   body:             [String] — any-of ILIKE contains against body (HTML column).
#   category:         [String] — any-of equality against the category enum.
#   email_account_id: String   — restrict to one account in the workspace.
#   has_attachment:   true     — restrict to messages with attachments.
#
# Actions: tag (via email_rule_tags), archive (boolean), mark_read (boolean),
# mail_folder (FK to MailFolder).  At least one action must be configured.
class EmailRule < ApplicationRecord
  belongs_to :workspace
  belongs_to :created_by, class_name: "User", optional: true
  belongs_to :mail_folder, optional: true

  has_many :email_rule_tags, dependent: :destroy
  has_many :tags, through: :email_rule_tags

  has_many :runs, class_name: "EmailRuleRun", dependent: :destroy

  scope :enabled, -> { where(enabled: true) }

  validates :name, presence: true
  validate :criteria_present
  validate :action_present
  validate :tags_in_workspace
  validate :folder_in_workspace

  before_validation :normalize_criteria

  # Array-valued criterion keys that are stored as arrays and normalised before
  # validation.  Address keys (from, to) are additionally downcased.
  ARRAY_CRITERION_KEYS = %w[from to subject body category].freeze
  ADDRESS_KEYS = %w[from to].freeze
  private_constant :ARRAY_CRITERION_KEYS, :ADDRESS_KEYS

  private

  def normalize_criteria
    return unless criteria.is_a?(Hash)

    normalised = {}

    ARRAY_CRITERION_KEYS.each do |key|
      next unless criteria.key?(key)

      values = Array(criteria[key])
        .flat_map { |v| v.to_s.split(",") }
        .map(&:strip)
        .reject(&:blank?)

      values = values.map(&:downcase) if ADDRESS_KEYS.include?(key)

      normalised[key] = values if values.any?
    end

    if (acct = criteria["email_account_id"].to_s.strip).present?
      normalised["email_account_id"] = acct
    end

    normalised["has_attachment"] = true if criteria["has_attachment"] == true

    self.criteria = normalised
  end

  # The account selector only narrows scope — it is not a condition.  Without
  # this distinction, an account-only rule would match that account's entire
  # mailbox (catastrophic when combined with the archive action).
  def criteria_present
    conditions = criteria.except("email_account_id")
      .reject { |_, v| v.nil? || (v.is_a?(Array) && v.empty?) || v == false }
    return if conditions.any?

    errors.add(:criteria, "at least one condition is required (a rule with no conditions would match all mail)")
  end

  def action_present
    has_action = tags.any? || archive? || mark_read? || mail_folder_id.present?
    errors.add(:base, "at least one action (tag, archive, mark read, or folder) is required") unless has_action
  end

  def tags_in_workspace
    return unless workspace_id.present? && tags.any?

    offending = tags.reject { |t| t.workspace_id == workspace_id }
    errors.add(:tags, "must all belong to the same workspace as the rule") if offending.any?
  end

  def folder_in_workspace
    return if mail_folder.nil?

    errors.add(:mail_folder, "must belong to the same workspace as the rule") if mail_folder.workspace_id != workspace_id
  end
end
