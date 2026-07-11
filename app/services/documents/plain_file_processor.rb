module Documents
  # Handles files that aren't business documents to classify — calendar invites, raw
  # emails, archives, html (Document::NON_DOCUMENT_CONTENT_TYPES). No LLM/OCR call:
  # give the file a readable filename-based title and mark the AI lane :skipped, the
  # purpose-built terminal state (re-runnable later via Files::UploadsController#analyze).
  # These are already excluded from the review queue (#reviewable_attachment), so a
  # .zip or .ics never costs a vision call or clutters review.
  class PlainFileProcessor
    def initialize(document)
      @document = document
    end

    def call
      # Safe/idempotent — derives a title from the filename when there's no AI metadata.
      Documents::TitleGenerator.new(@document).call

      @document.ai_skipped!

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
      Rails.logger.error("[Documents::PlainFileProcessor] Error processing document #{@document.id}: #{e.message}")
      @document.update!(ai_status: :failed, ai_error: e.message)
      raise
    end
  end
end
