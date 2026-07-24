module Emails
  # Downloads a message's provider attachments into email.files, rewrites cid:
  # references, and turns real (non-inline) attachments into Documents (deduped
  # by content hash; DocumentProcessJob enqueued per new Document). Extracted
  # verbatim from EmailProcessJob#process_attachments so the attachment-flag
  # backfill can ingest attachments for already-processed mail without
  # re-running triage, rules, or any other part of the ingest pipeline.
  class AttachmentIngester
    def self.call(email, mail_client = nil)
      new(email, mail_client || email.email_account.mail_client).call
    end

    def initialize(email, mail_client)
      @email = email
      @mail_client = mail_client
    end

    def call
      email = @email
      attachments = @mail_client.list_message_attachments(email.provider_message_id, email.provider_folder_id)
      count = 0
      cid_map = {}

      attachments.each do |att|
        next if att["attachmentId"].blank?

        raw_data = @mail_client.download_attachment(email.provider_message_id, email.provider_folder_id, att["attachmentId"])
        next if raw_data.nil? || raw_data.empty?

        filename = att["attachmentName"] || att["fileName"] || "attachment"
        content_type = att["mimeType"].presence || mime_type_for(filename)

        non_inline_attachment = att["attachmentType"] != "inline" && att["contentId"].blank?

        # Tracking pixels and spacer/icon images (1x1 and other sub-document sizes)
        # routinely arrive as plain, non-inline attachments with no Content-ID. They
        # are never real documents, and a 1x1 image makes the vision model reject the
        # whole analysis with a 400, so drop them before they're stored or turned into
        # a Document. Inline images (referenced from the body) are left untouched so
        # the message still renders.
        if non_inline_attachment && tracking_image?(raw_data, content_type)
          Rails.logger.info("[Emails::AttachmentIngester] Skipping tracking/degenerate image attachment for email #{email.id}: #{filename} (#{content_type})")
          next
        end

        attachment_record = email.files.attach(
          io: StringIO.new(raw_data),
          filename: filename,
          content_type: content_type
        )

        if att["contentId"].present? && attachment_record.present?
          blob = attachment_record.is_a?(Array) ? attachment_record.first&.blob : attachment_record.blob
          cid_map[att["contentId"]] = blob_path(blob) if blob
        end

        if non_inline_attachment
          if dmarc_report_email?(email) && dmarc_report_attachment?(filename, content_type)
            Rails.logger.info("[Emails::AttachmentIngester] Skipping DMARC report attachment for email #{email.id}: #{filename}")
            next
          end

          content_hash = Digest::SHA256.hexdigest(raw_data)
          existing = Document.find_by(content_hash: content_hash, email_account_id: email.email_account_id)

          if existing
            existing.document_email_messages.find_or_create_by!(email_message: email)
          else
            document = Document.new(
              source: :email,
              ai_status: :pending,
              review_status: :pending,
              document_type: :other,
              workspace: email.email_account.workspace,
              email_account: email.email_account,
              email_message_id: email.provider_message_id,
              content_hash: content_hash
            )
            document.original_file.attach(io: StringIO.new(raw_data), filename: filename, content_type: content_type)
            document.save!
            document.document_email_messages.create!(email_message: email)
            DocumentProcessJob.perform_later(document.id)
            count += 1
          end
        end
      rescue => e
        Rails.logger.error("[Emails::AttachmentIngester] Attachment error on email #{email.id}: #{e.message}")
      end

      if cid_map.any? && email.body.present?
        new_body = email.body.dup
        cid_map.each do |cid, url|
          new_body.gsub!("cid:#{cid}", url)
        end
        email.update!(body: new_body)
      end

      # &. — Sender-recorded outbound copies and backfilled messages have no scan log.
      email.email_scan_log&.increment!(:documents_created, count) if count > 0
      count
    end

    private

    def blob_path(blob)
      "/rails/active_storage/blobs/#{blob.signed_id}/#{blob.filename}"
    end

    def dmarc_report_email?(email)
      return false unless email.subject.present?
      email.subject.match?(/Report\s+Domain:/i) ||
        email.from_address.to_s.match?(/dmarc/i)
    end

    def dmarc_report_attachment?(filename, content_type)
      return false unless filename.present?
      ext = File.extname(filename.to_s).downcase
      return true if %w[.xml .gz .zip].include?(ext)
      content_type.to_s.match?(/xml/)
    end

    # Images this small in BOTH dimensions are tracking pixels, spacers, or
    # signature icons — never real document attachments.
    TRACKING_IMAGE_MAX_DIMENSION = 32

    def tracking_image?(raw_data, content_type)
      return false unless content_type.to_s.start_with?("image/")

      width, height = Images::Dimensions.read(raw_data)
      if width && height
        width <= TRACKING_IMAGE_MAX_DIMENSION && height <= TRACKING_IMAGE_MAX_DIMENSION
      else
        # Unrecognised/truncated header: only treat trivially small blobs as junk.
        raw_data.bytesize <= 1024
      end
    end

    def mime_type_for(filename)
      case File.extname(filename.to_s).downcase
      when ".pdf" then "application/pdf"
      when ".png" then "image/png"
      when ".jpg", ".jpeg" then "image/jpeg"
      when ".gif" then "image/gif"
      when ".doc" then "application/msword"
      when ".docx" then "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
      when ".xls" then "application/vnd.ms-excel"
      when ".xlsx" then "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
      when ".zip" then "application/zip"
      else "application/octet-stream"
      end
    end
  end
end
