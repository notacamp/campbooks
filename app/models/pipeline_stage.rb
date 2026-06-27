# frozen_string_literal: true

class PipelineStage < ApplicationRecord
  belongs_to :pipeline, touch: true

  has_many :memberships, class_name: "PipelineMembership", foreign_key: :current_stage_id,
           dependent: :nullify, inverse_of: :current_stage

  validates :name, presence: true, uniqueness: { scope: :pipeline_id }
  validates :position, presence: true, numericality: { only_integer: true }

  scope :ordered, -> { order(:position) }
  scope :terminal, -> { where(is_terminal: true) }
  scope :non_terminal, -> { where(is_terminal: false) }
end
