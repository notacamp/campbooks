# frozen_string_literal: true

module Pipelineable
  extend ActiveSupport::Concern

  included do
    has_many :pipeline_memberships, as: :item, dependent: :destroy
    has_many :pipelines, through: :pipeline_memberships
    has_many :pipeline_stages, through: :pipeline_memberships, source: :current_stage
  end

  def current_pipeline
    pipeline_memberships.includes(:pipeline).first&.pipeline
  end

  def current_stage_for(pipeline)
    pipeline_memberships.find_by(pipeline_id: pipeline.id)&.current_stage
  end

  def assign_to_pipeline!(pipeline, user: nil)
    membership = pipeline_memberships.find_or_create_by!(pipeline: pipeline) do |pm|
      pm.current_stage = pipeline.entry_stage
    end
    if membership.current_stage.nil?
      membership.update!(current_stage: pipeline.entry_stage, last_moved_at: Time.current)
    end
    membership
  end
end
