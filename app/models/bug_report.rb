# A user-submitted bug report. Always persisted locally (so a report is never
# lost), then mirrored to a GitHub issue by BugReportGithubSyncJob when a
# GITHUB_TOKEN + repo are configured. The capture context (URL, viewport,
# console errors, …) lives in `metadata`; an optional page screenshot is held
# as an Active Storage attachment.
class BugReport < ApplicationRecord
  belongs_to :workspace
  belongs_to :user

  has_one_attached :screenshot

  enum :status, { open: 0, triaged: 1, resolved: 2, closed: 3 }, default: :open

  validates :description, presence: true, length: { maximum: 5_000 }

  scope :recent, -> { order(created_at: :desc) }

  # A concise, single-line title derived from the report body — used as the
  # GitHub issue title. Falls back to the record id when the body is blank.
  def issue_title
    summary = description.to_s.strip.lines.first.to_s.strip
    summary.present? ? summary.truncate(80, omission: "…") : "Bug report ##{id}"
  end

  def synced_to_github?
    github_issue_number.present?
  end

  # Safe reader for a captured-context value, regardless of how `metadata` was
  # stored (string/symbol keys, or a nil column on legacy rows).
  def context(key)
    return nil unless metadata.is_a?(Hash)

    metadata[key.to_s]
  end
end
