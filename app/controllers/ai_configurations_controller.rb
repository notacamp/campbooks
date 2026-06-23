class AiConfigurationsController < ApplicationController
  def update
    @config = current_user.workspace.ai_configurations.find_or_initialize_by(purpose: params[:id])

    if config_params[:ai_adapter_id].present?
      @config.assign_attributes(config_params)
      if @config.save
        redirect_to settings_ai_path, success: t(".assignment_updated", purpose: @config.purpose_label)
      else
        redirect_to settings_ai_path, error: t(".failed_to_save", purpose: @config.purpose_label, errors: @config.errors.full_messages.to_sentence)
      end
    else
      # No adapter selected — remove the assignment (revert to no AI for this purpose)
      if @config.persisted?
        @config.destroy!
        redirect_to settings_ai_path, success: t(".reset_to_unassigned", purpose: @config.purpose_label)
      else
        redirect_to settings_ai_path, success: t(".is_unassigned", purpose: @config.purpose_label)
      end
    end
  end

  private

  def config_params
    params.require(:ai_configuration).permit(:ai_adapter_id, :enabled, :model, :max_tokens, :temperature)
  end
end
