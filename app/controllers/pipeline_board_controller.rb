# frozen_string_literal: true

class PipelineBoardController < ApplicationController
  before_action -> { require_entitlement!(:pipelines) }
  before_action :require_authentication
  before_action :set_pipeline

  COLUMN_LIMIT = 50

  def index
    @columns = build_columns
    render layout: false
  end

  def move
    membership = @pipeline.memberships.find_by!(id: params[:membership_id])
    new_stage = @pipeline.stages.find(params[:to_stage_id])

    if new_stage.nil? || new_stage == membership.current_stage
      render json: { ok: false }, status: :unprocessable_entity
      return
    end

    membership.move_to!(new_stage, user: Current.user)
    render json: { ok: true }
  rescue ActiveRecord::RecordNotFound
    render json: { ok: false }, status: :not_found
  end

  private

  def set_pipeline
    @pipeline = Current.workspace.pipelines.find(params[:id])
  end

  def build_columns
    @pipeline.stages.ordered.map do |stage|
      memberships = @pipeline.memberships
        .in_stage(stage.id)
        .includes(:item)
        .limit(COLUMN_LIMIT + 1)

      {
        stage: stage,
        memberships: memberships.first(COLUMN_LIMIT),
        has_more: memberships.size > COLUMN_LIMIT,
        draggable: !stage.is_terminal?
      }
    end
  end
end
