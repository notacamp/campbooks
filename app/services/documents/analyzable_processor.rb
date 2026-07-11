module Documents
  # The AI/OCR analysis pipeline for content-bearing documents (pdf, image, office).
  # Unchanged from the pre-split Documents::Processor — the dispatcher now only routes
  # analyzable files here, so the analyzer no longer has to classify non-documents.
  class AnalyzableProcessor
    def initialize(document)
      @document = document
    end

    def call
      @document.ai_processing!

      # Step 1: AI analysis
      result = Ai::DocumentAnalyzer.new(@document).call

      # AI failed (API error, no credits, unreadable, parse error). The analyzer has
      # already recorded ai_status: :failed + ai_error — surface it on the "AI broke"
      # lane (not the review queue, which is for approving real classifications).
      if result[:error]
        Rails.logger.warn("[Documents::AnalyzableProcessor] AI analysis failed for document #{@document.id}: #{result[:error]}")
        Notifier.document_failed(@document)
        return @document
      end

      # Step 2: PDF conversion if needed
      if @document.needs_pdf_conversion?
        Pdf::ImageConverter.new(@document).call
      end

      # Step 3: Generate canonical filename
      @document.generate_canonical_filename!

      # Step 4: Improve title when the AI result is still vague (email address, raw
      # filename, single word, etc.).  Safe/idempotent — no-ops when the title is
      # already descriptive, and swallows its own errors.
      Documents::TitleGenerator.new(@document).call

      # AI did its part; every completed document now awaits human sign-off — surface it.
      Notifier.documents_need_review(@document.workspace) if @document.review_pending?

      Events.publish(
        "document.processed",
        subject: @document,
        actor: nil,
        payload: {
          "filename" => @document.original_file&.filename.to_s,
          "document_type" => (@document.classification&.name || @document.document_type)
        }
      )

      @document
    rescue => e
      Rails.logger.error("[Documents::AnalyzableProcessor] Error processing document #{@document.id}: #{e.message}")
      @document.update!(ai_status: :failed, ai_error: e.message)
      Notifier.document_failed(@document)
      raise
    end
  end
end
