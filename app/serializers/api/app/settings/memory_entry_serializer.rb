# frozen_string_literal: true

module Api
  module App
    module Settings
      # One Scout memory entry — a derived sentence with origin and actions.
      # Entries are rebuilt per request from Scout::Memory::Catalog#entries.
      class MemoryEntrySerializer
        def initialize(entry)
          @entry = entry
        end

        def as_json(*)
          {
            id: @entry.id,
            facet: @entry.facet,
            sentence: sentence_data,
            origin: @entry.origin,
            origin_detail: @entry.origin_detail,
            actions: @entry.actions,
            form_path: @entry.form_path
          }
        end

        private

        def sentence_data
          s = @entry.sentence
          return s.to_s unless s.respond_to?(:plain)

          {
            plain: s.plain,
            html: s.respond_to?(:html) ? s.html : s.plain
          }
        end
      end
    end
  end
end
