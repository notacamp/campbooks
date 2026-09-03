# frozen_string_literal: true

module Scout
  module Memory
    module Sources
      # Document types as filing sentences: "Documents that look like **Invoice**
      # go to Paper › Invoice." The six SetupPresets builtins read as `default`
      # (Scout ships with them); anything you created reads as `taught`.
      class DocumentTypes < Base
        def entries
          workspace.document_types.order(:name).map { |type| entry_for(type) }
        end

        private

        def entry_for(type)
          # SetupPresets keys are space-separated ("bank statement"); stored type
          # names may be underscored ("bank_statement") — normalise before matching.
          builtin = SetupPresets.document_type(type.name.to_s.tr("_", " ")).present?
          key = type.auto_star ? "sentence_starred" : "sentence"

          build(
            id: "doctype:#{type.id}",
            facet: :filing,
            sentence: sentence("scout_memory.sources.document_types.#{key}", name: type.name.to_s.humanize),
            origin: builtin ? :default : :taught,
            origin_detail: builtin ? I18n.t("scout_memory.origins.default") : taught_detail(type.created_at),
            record: type,
            form_path: routes.settings_inbox_section_path("document_types"),
            actions: %i[edit]
          )
        end
      end
    end
  end
end
