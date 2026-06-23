class PdfGenerationJob < ApplicationJob
  queue_as :default

  def perform(document_id)
    document = Document.find(document_id)
    return unless document.needs_pdf_conversion?

    Pdf::ImageConverter.new(document).call
  end
end
