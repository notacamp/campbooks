# frozen_string_literal: true

module Api
  module App
    # Extends Api::V1::DocumentSerializer with the extra fields the SPA needs:
    # settle state, star, source context, push-to-Drive/Notion status, AI summary,
    # and a skim_card sub-hash (for Document Skim surfaces). Pass detail: true to
    # include the file info and extraction data; pass skim: true for the skim card.
    class DocumentSerializer
      include Rails.application.routes.url_helpers

      def initialize(document, detail: false, skim: false)
        @document = document
        @detail   = detail
        @skim     = skim
      end

      def as_json # rubocop:disable Metrics/MethodLength
        data = {
          id:                @document.id,
          title:             @document.display_title,
          document_type:     @document.document_type,
          document_type_id:  @document.document_type_id,
          ai_status:         @document.ai_status,
          review_status:     @document.review_status,
          source:            @document.source,
          starred:           @document.starred?,
          settled:           settled?,
          settled_at:        @document.settled_at&.iso8601,
          document_date:     @document.document_date&.iso8601,
          vendor_name:       @document.vendor_name,
          client_name:       @document.client_name,
          invoice_number:    @document.invoice_number,
          amount_cents:      @document.amount_cents,
          currency:          @document.try(:currency),
          description:       @document.description,
          canonical_filename: @document.canonical_filename,
          created_at:        @document.created_at.iso8601,
          source_email:      source_email_context,
          has_drive_config:  has_drive_config?,
          has_notion_mapping: has_notion_mapping?
        }

        if @detail
          data[:file]       = file_info
          data[:extraction] = @document.ai_extraction_data
          data[:ai_summary] = @document.try(:ai_summary)
        end

        data[:skim_card] = skim_card if @skim

        data
      end

      private

      def settled?
        @document.respond_to?(:settled?) ? @document.settled? : false
      end

      def source_email_context
        email = @document.email_messages&.first
        return nil unless email

        { id: email.id, subject: email.subject, from_address: email.from_address }
      end

      def has_drive_config?
        @document.classification&.google_drive_config.present?
      end

      def has_notion_mapping?
        @document.notion_database_mapping.present?
      end

      def file_info
        return nil unless @document.original_file.attached?

        blob = @document.original_file
        {
          filename:     blob.filename.to_s,
          content_type: blob.content_type,
          byte_size:    blob.byte_size,
          download_path: file_api_app_document_path(@document)
        }
      end

      def skim_card
        {
          entity_display_name: @document.entity_display_name,
          reference_display:   @document.try(:reference_display),
          amount_display:      @document.amount&.format,
          ai_confidence_score: @document.ai_confidence_score,
          is_image: @document.try(:image?),
          is_pdf:   @document.try(:pdf?),
          extracted_fields: ::Documents::ExtractedFieldSet.new(@document).fields
        }
      end
    end
  end
end
