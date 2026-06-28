# frozen_string_literal: true

class PipelineMembership < ApplicationRecord
  belongs_to :pipeline
  belongs_to :item, polymorphic: true
  belongs_to :current_stage, class_name: "PipelineStage", optional: true

  validates :item_id, uniqueness: { scope: %i[pipeline_id item_type] }
  validates :item_type, inclusion: { in: %w[Document EmailMessage] }

  before_create :set_timestamps
  before_save :track_stage_change, if: :current_stage_id_changed?

  scope :ordered, -> { order(:position) }
  scope :in_stage, ->(stage_id) { where(current_stage_id: stage_id) }
  scope :for_type, ->(type) { where(item_type: type) }

  # Moves the item into +new_stage+, recording the transition and firing the
  # pipeline.stage_entered event. A no-op when the stage is unchanged or nil, so
  # callers can invoke it idempotently.
  def move_to!(new_stage, user: nil)
    return if new_stage.nil? || new_stage.id == current_stage_id

    update!(current_stage_id: new_stage.id, last_moved_at: Time.current)
    fire_stage_entered_event(new_stage, user: user)
  end

  private

  def set_timestamps
    self.entered_at ||= Time.current
    self.last_moved_at ||= Time.current
  end

  def track_stage_change
    return unless current_stage_id.present? && current_stage_id_changed?

    history = stage_history.presence || []
    history[-1]["exited_at"] = Time.current.iso8601 if history.any? && history[-1]["exited_at"].nil?

    entry = {
      "stage_id" => current_stage_id,
      "stage_name" => current_stage&.name,
      "entered_at" => Time.current.iso8601,
      "exited_at" => nil
    }
    self.stage_history = history + [ entry ]
  end

  def fire_stage_entered_event(new_stage, user: nil)
    Events.publish("pipeline.stage_entered",
      subject: item,
      actor: user || :current,
      payload: {
        pipeline_name: pipeline.name,
        stage_name: new_stage.name,
        stage_id: new_stage.id,
        pipeline_id: pipeline.id
      }
    )
  end
end
