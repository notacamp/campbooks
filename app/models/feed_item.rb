# A single card in the home feed.
#
# The feed is a *materialized spine*: one row per surfaced item per user. A
# registry of Feed::Source classes populates the table; the heavy content
# (subject, excerpt, suggested actions) is rendered live from the referenced
# `subject` record, so a row only carries routing (kind + subject), ordering
# (sort_at, score, attention) and per-user state (seen / dismissed / acted).
#
# This keeps reads to a single indexed table (fast, paginatable far back in
# time) while content never goes stale. A Feed::Source#still_valid? check on
# read is the safety net for rows a refresh hasn't reconciled yet.
class FeedItem < ApplicationRecord
  belongs_to :user
  belongs_to :workspace
  belongs_to :subject, polymorphic: true

  # Item kinds. Deliberately strings, not a DB enum: a new Feed::Source can add
  # a kind without a migration — the source registry owns the list, not the schema.
  KINDS = %w[calendar_event starred_email email_action reply_reminder tag_suggestion reminder follow_up].freeze

  scope :for_user,      ->(user) { where(user: user) }
  scope :active,        -> { where(dismissed_at: nil, acted_at: nil) }
  scope :attention,     -> { where(attention: true) }
  scope :timeline,      -> { where(attention: false) }
  scope :ranked,        -> { order(score: :desc, sort_at: :desc) }
  scope :chronological, -> { order(sort_at: :desc, id: :desc) }

  def active?    = dismissed_at.nil? && acted_at.nil?
  def seen?      = seen_at.present?
  def dismissed? = dismissed_at.present?
  def acted?     = acted_at.present?

  # State stamps use update_column: they're simple per-user flags with no
  # validations or callbacks to run, and we don't want to touch updated_at.
  def mark_seen!
    return if seen?
    update_column(:seen_at, Time.current)
  end

  def dismiss!
    update_column(:dismissed_at, Time.current)
  end

  def mark_acted!
    update_column(:acted_at, Time.current)
  end

  # Undo: return a resolved card to the feed (clears the action/dismissal stamps).
  def reactivate!
    update_columns(acted_at: nil, dismissed_at: nil, updated_at: Time.current)
  end
end
