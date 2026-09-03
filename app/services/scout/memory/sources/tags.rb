# frozen_string_literal: true

module Scout
  module Memory
    module Sources
      # Tags as filing sentences. A tag with a classification prompt becomes
      # "Tag mail about **X** as **#name**." (taught). A hidden provider label
      # becomes "Provider label **X** stays hidden." — learned when Scout hid it
      # (classified_at set by the AI classifier), taught when you hid it yourself.
      class Tags < Base
        def entries
          workspace.tags.order(:name).flat_map { |tag| entries_for(tag) }
        end

        # Confirm an AI-hidden label: record the human sign-off on the existing
        # classification_reason string (no new column, per the spec).
        def confirm(entry)
          tag = entry.record
          return false unless tag.is_a?(Tag) && tag.workspace_id == workspace.id

          tag.update(classification_reason: "confirmed")
        end

        private

        def entries_for(tag)
          if tag.hidden?
            [ hidden_entry(tag) ]
          elsif (topic = first_sentence(tag.prompt)).present?
            # Tag prompts are free-text classification hints and can be long — keep
            # the memory sentence to a readable phrase.
            [ topic_entry(tag, topic.truncate(60)) ]
          else
            []
          end
        end

        def topic_entry(tag, topic)
          build(
            id: "tag:#{tag.id}",
            facet: :filing,
            sentence: sentence("scout_memory.sources.tags.topic", topic: topic, tag: "##{tag.name}"),
            origin: :taught,
            origin_detail: taught_detail(tag.created_at),
            record: tag,
            form_path: routes.settings_inbox_section_path("tags"),
            actions: %i[edit]
          )
        end

        def hidden_entry(tag)
          learned = tag.classified_at.present?
          build(
            id: "taghidden:#{tag.id}",
            facet: :filing,
            sentence: sentence("scout_memory.sources.tags.hidden", name: tag.name),
            origin: learned ? :learned : :taught,
            origin_detail: learned ? I18n.t("scout_memory.origins.learned_labels") : taught_detail(tag.created_at),
            record: tag,
            form_path: routes.settings_inbox_section_path("tags"),
            actions: learned ? %i[confirm] : %i[edit]
          )
        end
      end
    end
  end
end
