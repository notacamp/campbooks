module Documents
  # Learns from the human-approved corpus. When the AI is about to classify a new
  # document, we look at how the workspace's *approved* documents from the same sender
  # (or with a similar filename) were classified, and surface the dominant type as a
  # hint. There is no training data or rules table — the approved documents themselves
  # ARE the memory, so every approval/correction a human makes improves the next guess.
  #
  # Used by Ai::DocumentAnalyzer to (a) bias the prompt toward the learned type and
  # (b) record what was matched into ai_extraction_data["classification_memory"] so the
  # human can see *why* a type was suggested.
  #
  # The consensus/cascade machinery now lives in the generic Learning:: substrate
  # (Learning::Memory + Learning::Sources::Documents); this class is the document-facing
  # adapter that resolves the winning DocumentType id to a name and formats the hint.
  class ClassificationMemory
    # The document-facing suggestion the analyzer consumes: it needs the resolved
    # type_name in addition to the generic label/count/total.
    Suggestion = Data.define(:document_type_id, :type_name, :source, :count, :total)

    def initialize(document)
      @document = document
    end

    # The strongest available signal, or nil. Sender consensus beats filename — who
    # sent it is a stronger prior than what it happens to be named (the ordering lives
    # in Learning::Sources::Documents#signal_cascade).
    def suggestion
      return @suggestion if defined?(@suggestion)
      @suggestion = resolve
    end

    # A one-line natural-language hint to bias the AI prompt, or nil when we've not
    # seen enough approved examples to say anything useful.
    def prompt_hint
      s = suggestion
      return nil unless s

      Learning::Strategies::PromptHint.for_documents(s, type_name: s.type_name)
    end

    private

    # Run the generic engine, then resolve the raw DocumentType id (the engine's
    # domain-opaque `label`) to a human name. A vanished type → no suggestion.
    def resolve
      raw = Learning::Memory.new(source: Learning::Sources::Documents.new(@document)).suggestion
      return nil unless raw

      name = DocumentType.where(id: raw.label).pick(:name)
      return nil if name.blank?

      Suggestion.new(document_type_id: raw.label, type_name: name,
                     source: raw.source, count: raw.count, total: raw.total)
    end
  end
end
