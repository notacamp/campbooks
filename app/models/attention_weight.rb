# frozen_string_literal: true

# One learned attention weight per (user, counterpart): how much a Person or an
# Organization matters to this user, from what they do with that counterpart's
# mail, meetings and money — materialized by Attention::Refresh so every
# surface reads one number instead of recomputing relevance its own way.
class AttentionWeight < ApplicationRecord
  belongs_to :user
  belongs_to :workspace
  belongs_to :subject, polymorphic: true

  SUBJECT_TYPES = %w[Person Organization].freeze
  validates :subject_type, inclusion: { in: SUBJECT_TYPES }
  validates :weight, :confidence, numericality: { greater_than_or_equal_to: 0, less_than_or_equal_to: 1 }

  scope :for_user, ->(user) { where(user: user) }
  scope :ranked,   -> { order(weight: :desc, confidence: :desc, id: :asc) }

  def person? = subject_type == "Person"
  def organization? = subject_type == "Organization"

  # Reasons as Attention::Reason values (strongest first).
  def reason_values = Array(reasons).map { |r| Attention::Reason.from_h(r) }
end
