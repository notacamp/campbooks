module Documents
  # Routes a document to the right processor. Content-bearing files we can analyze
  # (pdf, image, office → AI/OCR) go to AnalyzableProcessor; non-documents (calendar
  # invites, raw emails, archives, html — Document::NON_DOCUMENT_CONTENT_TYPES) go to
  # PlainFileProcessor, which stores them deterministically and never pays for an
  # LLM/OCR call. Splitting the two keeps a .zip or .ics off the vision path and lets
  # the analyzer prompt drop its "junk file → other" special-casing.
  class Processor
    def initialize(document)
      @document = document
    end

    def call
      strategy = @document.analyzable? ? AnalyzableProcessor : PlainFileProcessor
      strategy.new(@document).call
    end
  end
end
