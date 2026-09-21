# frozen_string_literal: true

module Api
  module App
    module Settings
      # Custom AI guidance per feature purpose. Clearing the instructions field
      # restores the built-in prompt by deleting the row.
      class AiPromptsController < Api::App::BaseController
        before_action :set_entry, only: %i[show update]

        # GET /api/app/settings/ai_prompts
        def index
          entries = Ai::PromptCatalog.all
          prompts = current_workspace.ai_prompts.index_by(&:purpose)

          render_data(
            entries.map do |entry|
              prompt = prompts[entry.key]
              prompt_data(entry, prompt)
            end
          )
        end

        # GET /api/app/settings/ai_prompts/:purpose
        def show
          prompt = find_or_build
          render_data prompt_data(@entry, prompt.persisted? ? prompt : nil)
        end

        # PATCH /api/app/settings/ai_prompts/:purpose
        def update
          prompt = find_or_build
          instructions = params.dig(:ai_prompt, :instructions).to_s.strip

          if instructions.blank?
            prompt.destroy if prompt.persisted?
            render_data prompt_data(@entry, nil)
          else
            prompt.instructions = instructions
            if prompt.save
              render_data prompt_data(@entry, prompt)
            else
              render_error("invalid", prompt.errors.full_messages.to_sentence, status: :unprocessable_entity)
            end
          end
        end

        private

        def set_entry
          @entry = Ai::PromptCatalog.find(params[:purpose])
          render_not_found unless @entry
        end

        def find_or_build
          current_workspace.ai_prompts.find_or_initialize_by(purpose: @entry.key)
        end

        def prompt_data(entry, prompt)
          {
            purpose: entry.key,
            label: entry.label,
            description: entry.respond_to?(:description) ? entry.description : nil,
            instructions: prompt&.instructions,
            configured: prompt&.persisted? && prompt&.configured?
          }
        end
      end
    end
  end
end
