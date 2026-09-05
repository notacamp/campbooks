# frozen_string_literal: true

# One materialized row in the People directory per (user, counterpart). Computed
# by People::Standings.refresh! (People::StandingsRefreshJob) so the People list
# reads a paginated table instead of recomputing standings on every request.
#
# `counterpart_type` is "Person" or "Organization". Display columns (name,
# subtitle, avatar_email, avatar_initial) are snapshot-copied from the live
# record at refresh time so each row renders without touching other tables.
class PeopleStanding < ApplicationRecord
  belongs_to :workspace
  belongs_to :user
  belongs_to :counterpart, polymorphic: true
  belongs_to :email_thread, optional: true

  STANDING_KINDS = %w[attention summary last_exchange none].freeze
  validates :standing_kind, inclusion: { in: STANDING_KINDS }
  validates :name, presence: true

  scope :for_user,  ->(user) { where(user: user) }
  scope :needing,   -> { where(needs_you: true) }
  scope :recent,    -> { where(needs_you: false) }
  # Primary sort: score descending, then persons before their org on ties (Person > Organization
  # lexicographically), then livelier row, then id for stable pagination.
  scope :ranked,    -> { order(score: :desc, counterpart_type: :desc, last_activity_at: :desc, id: :asc) }
  # The inbox order: newest activity first, rows without activity last, stable by id.
  scope :latest,    -> { order(Arel.sql("last_activity_at DESC NULLS LAST"), id: :asc) }
  scope :search, lambda { |query|
    like = "%#{sanitize_sql_like(query.to_s.strip)}%"
    where("name ILIKE :like OR subtitle ILIKE :like OR avatar_email ILIKE :like", like: like)
  }

  def person? = counterpart_type == "Person"
  def organization? = counterpart_type == "Organization"

  # Rebuild a People::Standing::Result from the stored columns so callers that
  # need the result object don't have to recompute it live.
  def standing
    People::Standing::Result.new(
      detail: detail,
      detail_kind: detail_kind&.to_sym,
      money: (data || {})["money"],
      needs_you: needs_you,
      thread_id: email_thread_id,
      overdue_days: wait_days,
      kind: standing_kind.to_sym,
      verb: verb&.to_sym,
      subject: subject,
      wait_days: wait_days,
      feed_item_id: feed_item_id,
      email_message_id: email_message_id
    )
  end

  # Build a People::Counterpart from the stored display columns. No additional
  # queries: record is nil, and the score is reconstructed from persisted fields.
  def to_counterpart
    People::Counterpart.new(
      id: counterpart_id,
      kind: person? ? :person : :organization,
      record: nil,
      name: name,
      subtitle: subtitle,
      avatar_email: avatar_email,
      avatar_initial: avatar_initial,
      last_activity: last_activity_at,
      standing: standing,
      facts: nil,
      score: People::Priority::Score.new(value: score, needs_you: needs_you, strength: strength, recency: 0.0),
      data: data || {}
    )
  end
end
