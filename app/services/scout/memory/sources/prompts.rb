# frozen_string_literal: true

module Scout
  module Memory
    module Sources
      # AI reading guidance (AiPrompt) as Style sentences. A workspace custom
      # prompt reads "When reading <purpose>, also: **<first line>**." (taught);
      # each catalog purpose without one reads "<purpose> uses the built-in
      # guidance." (default). Edit opens the AI prompts page.
      class Prompts < Base
        def entries
          custom = workspace.ai_prompts.index_by { |prompt| prompt.purpose.to_s }

          Ai::PromptCatalog.all.map do |catalog_entry|
            prompt = custom[catalog_entry.key.to_s]
            if prompt&.instructions.present?
              custom_entry(catalog_entry, prompt)
            else
              default_entry(catalog_entry)
            end
          end
        end

        private

        def custom_entry(catalog_entry, prompt)
          build(
            id: "aiprompt:#{prompt.id}",
            facet: :style,
            sentence: sentence("scout_memory.sources.prompts.custom",
              purpose: catalog_entry.label, instruction: first_sentence(prompt.instructions)),
            origin: :taught,
            origin_detail: taught_detail(prompt.updated_at),
            record: prompt,
            form_path: routes.edit_settings_ai_prompt_path(catalog_entry.key),
            actions: %i[edit]
          )
        end

        def default_entry(catalog_entry)
          build(
            id: "prompt:#{catalog_entry.key}",
            facet: :style,
            sentence: sentence("scout_memory.sources.prompts.default", purpose: catalog_entry.label),
            origin: :default,
            origin_detail: I18n.t("scout_memory.origins.default"),
            record: nil,
            form_path: routes.edit_settings_ai_prompt_path(catalog_entry.key),
            actions: %i[edit]
          )
        end
      end
    end
  end
end
