# frozen_string_literal: true

module Scout
  module Memory
    module Sources
      # Base for a behaviour source: turns one family of records (email rules,
      # tags, learned skim decisions, …) into memory Entry value objects, and
      # knows how to confirm/remove the entries it owns. One instance per request,
      # constructed with the current workspace + user.
      class Base
        attr_reader :workspace, :user

        def initialize(workspace:, user:)
          @workspace = workspace
          @user = user
        end

        # Array<Entry>. Override.
        def entries
          []
        end

        # Confirm / remove a learned-or-taught entry this source owns. Default to
        # no-ops; sources override the ones they support. Return truthy on success
        # (the catalog re-renders the row) — false when unsupported or not allowed.
        def confirm(_entry) = false
        def remove(_entry) = false

        # e.g. Scout::Memory::Sources::EmailRules -> :email_rules
        def source_key
          self.class.name.demodulize.underscore.to_sym
        end

        private

        # Render a memory sentence from an i18n template under scout_memory.sources.
        # String values are stripped of any `**` so record data can never open or
        # close a bold run; numeric values (e.g. `count:` for pluralization) pass
        # through untouched. Bold structure lives entirely in the template
        # (`**%{x}**` and literal `**word**`).
        def sentence(key, **values)
          safe = values.transform_values { |v| v.is_a?(String) ? v.gsub("**", "") : v }
          Scout::Memory::Sentence.parse(I18n.t(key, **safe))
        end

        def build(id:, facet:, sentence:, origin:, **rest)
          Entry.new(id: id, facet: facet, sentence: sentence, origin: origin, source_key: source_key, **rest)
        end

        # First sentence of a longer free-text blob (a tag prompt, a stated
        # writing style), collapsed to one line for the memory sentence. Trailing
        # sentence punctuation is dropped so the surrounding template controls the
        # final period (no "voice.." doubling when the fragment sits mid-sentence
        # or before the template's own ".").
        def first_sentence(text)
          one = text.to_s.strip.split(/(?<=[.!?])\s+/).first.to_s.strip.tr("\n", " ").squeeze(" ")
          one.sub(/[.!?]+\z/, "").presence
        end

        def routes
          Rails.application.routes.url_helpers
        end

        def taught_detail(time)
          return I18n.t("scout_memory.origins.taught") if time.blank?

          I18n.t("scout_memory.origins.taught_on", date: I18n.l(time.to_date, format: :day_month))
        end
      end
    end
  end
end
