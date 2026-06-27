# frozen_string_literal: true

class Settings::PipelinesController < Settings::BaseController
  before_action -> { require_entitlement!(:pipelines) }
  before_action :set_pipeline, only: %i[edit update destroy toggle]

  def index
    @pipelines = Current.workspace.pipelines.ordered.includes(:stages)
  end

  def new
    @pipeline = Current.workspace.pipelines.new
    @pipeline.stages.build(name: "New", color: "#6366f1", position: 1)
  end

  def create
    @pipeline = Current.workspace.pipelines.new(pipeline_params)
    if @pipeline.save
      redirect_to settings_pipelines_path, success: t(".created")
    else
      render :new, status: :unprocessable_entity
    end
  end

  def edit
  end

  def update
    if @pipeline.update(pipeline_params)
      redirect_to settings_pipelines_path, success: t(".updated")
    else
      render :edit, status: :unprocessable_entity
    end
  end

  def destroy
    @pipeline.destroy!
    redirect_to settings_pipelines_path, success: t(".deleted")
  end

  def toggle
    head :ok
  end

  private

  def set_pipeline
    @pipeline = Current.workspace.pipelines.find(params[:id])
  end

  def pipeline_params
    params.require(:pipeline).permit(
      :name, :description, :icon, :applies_to, :position,
      stages_attributes: %i[id name color position description is_terminal _destroy]
    )
  end
end
