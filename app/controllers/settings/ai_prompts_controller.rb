# Settings → AI Prompts, plus the per-resource "Customize AI" modal.
#
# Each customizable prompt is one Ai::PromptCatalog entry (keyed by `purpose`).
# The modal edits a single AiPrompt's `instructions`, which Ai::Configuration
# appends to the built-in prompt for that purpose. Clearing the box deletes the
# row and restores the default.
class Settings::AiPromptsController < Settings::BaseController
  before_action :set_entry, only: %i[edit update]

  def index
    @entries = Ai::PromptCatalog.all
    @prompts = Current.workspace.ai_prompts.index_by(&:purpose)
  end

  # Rendered into the shared setup-modal turbo-frame, from a resource page's
  # "Customize AI" button or the settings list.
  def edit
    render partial: "settings/ai_prompts/modal",
           locals: { entry: @entry, prompt: find_or_build }
  end

  def update
    prompt = find_or_build
    instructions = params.dig(:ai_prompt, :instructions).to_s.strip

    # Empty = revert to the built-in prompt: drop the row entirely.
    if instructions.blank?
      prompt.destroy if prompt.persisted?
      return respond_saved(cleared: true)
    end

    prompt.instructions = instructions
    if prompt.save
      respond_saved
    else
      render partial: "settings/ai_prompts/modal",
             locals: { entry: @entry, prompt: prompt }, status: :unprocessable_entity
    end
  end

  private

  # Keep the "AI Prompts" sidebar item highlighted across every action.
  def current_section = "ai_prompts"

  def set_entry
    @entry = Ai::PromptCatalog.find(params[:purpose])
    head :not_found unless @entry
  end

  def find_or_build
    Current.workspace.ai_prompts.find_or_initialize_by(purpose: @entry.key)
  end

  # Toast + close the modal + refresh the settings row. The row replace is a
  # harmless no-op on the resource pages, where that element isn't present.
  def respond_saved(cleared: false)
    saved = Current.workspace.ai_prompts.find_by(purpose: @entry.key)
    message = cleared ? t("ai_prompts.settings.cleared", name: @entry.label)
                      : t("ai_prompts.settings.saved", name: @entry.label)
    render turbo_stream: [
      notify_stream(message),
      turbo_stream.update("setup_modal_frame", partial: "shared/modals/close"),
      turbo_stream.replace("ai_prompt_row_#{@entry.key}",
        partial: "settings/ai_prompts/row",
        locals: { entry: @entry, prompt: saved })
    ]
  end
end
