# frozen_string_literal: true

module Api
  module V1
    # Serializes a Document for the public API. List responses carry the core
    # fields; pass detail: true (the show endpoint) to include the attached file
    # info (with a download path) and the raw AI extraction data.
    class DocumentSerializer
      include Rails.application.routes.url_helpers

      def initialize(document, detail: false)
        @document = document
        @detail = detail
      end

      def as_json
        data = {
          id: @document.id,
          title: @document.display_title,
          document_type: @document.document_type,
          document_type_id: @document.document_type_id,
          ai_status: @document.ai_status,
          review_status: @document.review_status,
          source: @document.source,
          starred: @document.starred,
          document_date: @document.document_date&.iso8601,
          vendor_name: @document.vendor_name,
          client_name: @document.client_name,
          invoice_number: @document.invoice_number,
          amount_cents: @document.amount_cents,
          currency: @document.try(:currency),
          description: @document.description,
          canonical_filename: @document.canonical_filename,
          created_at: @document.created_at.iso8601
        }

        if @detail
          data[:file] = file_info
          data[:extraction] = @document.ai_extraction_data
        end

        data
      end

      private

      def file_info
        return nil unless @document.original_file.attached?

        blob = @document.original_file
        {
          filename: blob.filename.to_s,
          content_type: blob.content_type,
          byte_size: blob.byte_size,
          # Path relative to the API host; fetch with the same bearer token.
          download_path: file_api_v1_document_path(@document)
        }
      end
    end
  end
end
