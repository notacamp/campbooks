# frozen_string_literal: true

class PipelineBoardController < ApplicationController
  include PipelineColumns

  # Viewing a board you already have is allowed even at the pipeline cap; only
  # creating new pipelines is limited (Settings::PipelinesController#create).
  before_action -> { require_entitlement!(:pipelines, ignore_limit: true) }
  before_action :set_pipeline

  def index
    @columns = build_columns(@pipeline)
  end

  def move
    membership = @pipeline.memberships.find(params[:membership_id])
    # A pipeline is workspace-scoped, but email access is per-user — never reveal
    # or move an email from an account this user can't read. 404, don't 403.
    raise ActiveRecord::RecordNotFound unless accessible_item?(membership.item)

    if membership.current_stage&.is_terminal?
      return render json: { ok: false, error: "terminal" }, status: :unprocessable_entity
    end

    new_stage = @pipeline.stages.find(params[:to_stage_id])
    membership.move_to!(new_stage, user: Current.user)
    render json: { ok: true }
  rescue ActiveRecord::RecordNotFound
    render json: { ok: false }, status: :not_found
  end

  private

  def set_pipeline
    @pipeline = Current.workspace.pipelines.find(params[:id])
  end
end
