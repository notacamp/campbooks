# frozen_string_literal: true

class PipelineStage < ApplicationRecord
  belongs_to :pipeline, touch: true

  has_many :memberships, class_name: "PipelineMembership", foreign_key: :current_stage_id,
           dependent: :nullify, inverse_of: :current_stage

  PALETTE = %w[#6366f1 #8b5cf6 #ec4899 #ef4444 #f59e0b #10b981 #06b6d4 #64748b].freeze

  validates :name, presence: true, uniqueness: { scope: :pipeline_id, case_sensitive: false }
  validates :position, presence: true, numericality: { only_integer: true }
  # Rendered into an inline `style` and concatenated with an alpha suffix
  # (color + "20"), so only a plain 6-digit hex is safe — reject anything else
  # (CSS-injection guard, and keeps the alpha trick valid).
  validates :color, format: { with: /\A#\h{6}\z/ }

  scope :ordered, -> { order(:position, :id) }
  scope :terminal, -> { where(is_terminal: true) }
  scope :non_terminal, -> { where(is_terminal: false) }
end
