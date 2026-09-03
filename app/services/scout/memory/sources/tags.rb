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
            [ topic_entry(tag, topic) ]
          else
            []
          end
        end

        # Prompts are usually written as a test ("The email contains content
        # related to accounting"), so the sentence reads "Tag mail as #accounting
        # when it contains content related to accounting." A prompt that isn't
        # phrased that way ("Invoices and receipts from vendors") reads "… when
        # it's about invoices and receipts from vendors."
        PROMPT_PREFIX = /\A(?:the\s+)?(?:e-?mails?|mail|messages?)\s+(is|are|contains?|relates?\s+to|concerns?|mentions?|includes?)\s+/i
        VERB_FORMS = { "are" => "is", "contain" => "contains", "relate to" => "relates to",
                       "concern" => "concerns", "mention" => "mentions", "include" => "includes" }.freeze

        def topic_entry(tag, topic)
          key, args = topic_phrase(topic.sub(/[.!]\z/, ""))
          build(
            id: "tag:#{tag.id}",
            facet: :filing,
            sentence: sentence("scout_memory.sources.tags.#{key}", tag: "##{tag.name}", **args),
            origin: :taught,
            origin_detail: taught_detail(tag.created_at),
            record: tag,
            form_path: routes.settings_inbox_section_path("tags"),
            actions: %i[edit]
          )
        end

        def topic_phrase(topic)
          if (match = topic.match(PROMPT_PREFIX))
            verb = match[1].downcase.squeeze(" ")
            verb = VERB_FORMS.fetch(verb, verb)
            rest = topic.sub(PROMPT_PREFIX, "").strip
            [ "topic_clause", { clause: "it #{verb} #{rest}".truncate(80) } ]
          else
            about = topic.dup
            about[0] = about[0].downcase
            [ "topic_about", { topic: about.truncate(70) } ]
          end
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
