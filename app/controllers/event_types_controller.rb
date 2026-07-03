class EventTypesController < ApplicationController
  before_action :require_authentication
  before_action :set_type, only: [ :edit, :update, :destroy ]

  def index
    @types = Current.workspace.event_types.order(:name).to_a
  end

  def new
    @type = Current.workspace.event_types.new
  end

  def create
    @type = Current.workspace.event_types.new(type_params)
    if @type.save
      respond_to do |format|
        format.turbo_stream
        format.html { redirect_to event_types_path, success: t(".success") }
      end
    else
      render_form_errors
    end
  end

  def edit; end

  def update
    if @type.update(type_params)
      respond_to do |format|
        format.turbo_stream
        format.html { redirect_to event_types_path, success: t(".success") }
      end
    else
      render_form_errors
    end
  end

  def destroy
    @type.destroy
    respond_to do |format|
      format.turbo_stream
      format.html { redirect_to event_types_path, success: t(".success") }
    end
  end

  # One-click starter set from the empty state. Skips names that already exist so
  # it's safe to re-run.
  def starters
    existing = Current.workspace.event_types.pluck(:name).map(&:downcase)
    EventType::STARTERS.each do |attrs|
      next if existing.include?(attrs[:name].downcase)
      Current.workspace.event_types.create(attrs)
    end
    redirect_to event_types_path, success: t(".success")
  end

  private

  def render_form_errors
    render turbo_stream: turbo_stream.update(
      "event_type_form",
      partial: "event_types/form",
      locals: { type: @type }
    ), status: :unprocessable_entity
  end

  def set_type
    @type = Current.workspace.event_types.find(params[:id])
  end

  def type_params
    params.require(:event_type).permit(:name, :icon, :prompt)
  end
end
