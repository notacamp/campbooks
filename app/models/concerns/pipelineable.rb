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

  # Places this item into +pipeline+ at its entry stage (idempotent — re-assigning
  # an item already in the pipeline leaves its current stage untouched). Moving it
  # into the entry stage fires pipeline.stage_entered, so workflows can react to an
  # item entering the pipeline. The retry covers the find_or_create_by! race on the
  # unique (pipeline, item) index.
  def assign_to_pipeline!(pipeline, user: nil)
    membership = pipeline_memberships.find_or_create_by!(pipeline: pipeline)
    membership.move_to!(pipeline.entry_stage, user: user) if membership.current_stage_id.nil?
    membership
  rescue ActiveRecord::RecordNotUnique
    retry
  end
end
