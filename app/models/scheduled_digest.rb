# frozen_string_literal: true

# A user-owned, workspace-scoped scheduled digest: a saved scope (multi-source),
# a schedule (rrule), optional AI summarization, and delivery configuration
# (email and/or home feed). Each occurrence materializes as a DigestIssue.
#
# Generalizes the hardcoded NeedsAttentionDigestJob/DigestMailer#needs_attention:
# users own and configure their own digests instead of toggling a single system one.
class ScheduledDigest < ApplicationRecord
  # Override model_name so routes/params/i18n read "digest" even though the class
  # cannot be named Digest (conflicts with Ruby stdlib Digest module).
  def self.model_name = ActiveModel::Name.new(self, nil, "Digest")

  # model_name override does NOT affect the table name — make it explicit so AR
  # never misreads the model_name plural ("digests") as the table name.
  self.table_name = "scheduled_digests"

  belongs_to :workspace
  belongs_to :user

  has_many :issues, class_name: "DigestIssue", dependent: :destroy, inverse_of: :scheduled_digest

  RRULE_FORMAT = /\AFREQ=(DAILY|WEEKLY|MONTHLY)(;INTERVAL=[0-9]+)?\z/

  validates :name, presence: true, length: { maximum: 80 }
  validates :rrule, presence: true, format: { with: RRULE_FORMAT }
  validates :next_run_at, presence: true
  validates :ai_instructions, length: { maximum: 2000 }, allow_blank: true
  validate :sources_config_valid

  scope :enabled, -> { where(enabled: true) }
  scope :due, ->(now = Time.current) { enabled.where(next_run_at: ..now) }

  # The configured sources as an array of hashes (string keys).
  def sources
    Array(config["sources"])
  end

  # Advance the schedule to the next occurrence after `from`, recording when the
  # last run happened. Called transactionally by DigestSweepJob before enqueuing
  # DigestRunJob so the sweep never double-enqueues for the same occurrence.
  def advance_schedule!(from: Time.current)
    update!(
      last_run_at: from,
      next_run_at: ScheduleCalculator.next_occurrence(next_run_at, rrule, from)
    )
  end

  # Parsed frequency symbol for display.
  def frequency
    case ScheduleCalculator.parse_rrule(rrule)[:freq]&.upcase
    when "DAILY"   then :daily
    when "WEEKLY"  then :weekly
    when "MONTHLY" then :monthly
    end
  end

  # Default lookback window used when computing period_start for the first run.
  def default_lookback
    case frequency
    when :daily   then 1.day
    when :weekly  then 7.days
    when :monthly then 31.days
    else 7.days
    end
  end

  private

  VALID_SOURCE_TYPES = Digests::Sources::KEYS

  def sources_config_valid
    srcs = Array(config["sources"])

    if srcs.empty? || srcs.size > 6
      errors.add(:config, "must have between 1 and 6 sources")
      return
    end

    srcs.each_with_index do |src, idx|
      next errors.add(:config, "source #{idx + 1} must be a hash") unless src.is_a?(Hash)

      type = src["type"].to_s
      unless VALID_SOURCE_TYPES.include?(type)
        errors.add(:config, "source #{idx + 1} has unknown type '#{type}'")
        next
      end

      validate_source_config(type, src, idx)
    end
  end

  def validate_source_config(type, src, idx)
    case type
    when "emails"
      query = src["query"].to_s
      if query.length > 200
        errors.add(:config, "source #{idx + 1}: query exceeds 200 characters")
      end
      parsed = Emails::SearchQuery.parse(query)
      if parsed.filters[:folder].present?
        errors.add(:config, "source #{idx + 1}: folder scoping is not supported in digests")
      end
    when "calendar", "tasks", "reminders"
      window = src["window_days"]
      unless [ 7, 14, 30 ].include?(window.to_i) || window.nil?
        errors.add(:config, "source #{idx + 1}: window_days must be 7, 14, or 30")
      end
    when "documents"
      types = Array(src["document_types"])
      if types.size > 10
        errors.add(:config, "source #{idx + 1}: document_types may not exceed 10 entries")
      end
    end
  end
end
