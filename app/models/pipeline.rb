# frozen_string_literal: true

class Pipeline < ApplicationRecord
  belongs_to :workspace

  has_many :stages, -> { order(:position) }, class_name: "PipelineStage", dependent: :destroy, inverse_of: :pipeline
  has_many :memberships, class_name: "PipelineMembership", dependent: :destroy

  enum :applies_to, { documents: 0, emails: 1, both: 2 }

  validates :name, presence: true, uniqueness: { scope: :workspace_id }
  validates :applies_to, presence: true

  accepts_nested_attributes_for :stages, allow_destroy: true, reject_if: ->(attrs) { attrs["name"].blank? }

  scope :ordered, -> { order(:position) }
  scope :for_documents, -> { where(applies_to: %i[documents both]) }
  scope :for_emails, -> { where(applies_to: %i[emails both]) }

  def entry_stage
    stages.ordered.first
  end
end
