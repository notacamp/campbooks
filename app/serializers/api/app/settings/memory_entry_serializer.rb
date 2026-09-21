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

        # Expose the sentence as `plain` (for text/search) plus `spans`
        # ([{text, bold}] runs) so the client can render bold emphasis with its
        # own escaping (<b> per run) — never as HTML. There is deliberately NO
        # `html` field: Scout::Memory::Sentence has no real/sanitized HTML, and
        # its text embeds user-derived values (tag names, taught text), so a
        # dangerouslySetInnerHTML-style consumer would be an XSS sink.
        def sentence_data
          s = @entry.sentence

          if s.respond_to?(:spans)
            { plain: s.plain, spans: s.spans }
          else
            text = s.to_s
            { plain: text, spans: [ { text: text, bold: false } ] }
          end
        end
      end
    end
  end
end
