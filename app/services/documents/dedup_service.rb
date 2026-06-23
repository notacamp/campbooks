module Documents
  class DedupService
    def initialize(dry_run: true)
      @dry_run = dry_run
      @stats = { backfilled: 0, groups: 0, merged: 0, deleted: 0, kept: 0 }
    end

    def call
      Rails.logger.info("[Documents::DedupService] Starting#{' (DRY RUN)' if @dry_run}")

      backfill_content_hashes
      find_and_merge_duplicates

      Rails.logger.info("[Documents::DedupService] Done: #{@stats.inspect}")
      @stats
    end

    private

    def backfill_content_hashes
      docs = Document.where(content_hash: nil).includes(:original_file_blob)
      count = docs.count
      Rails.logger.info("[Documents::DedupService] Backfilling content_hash for #{count} documents") if count > 0

      docs.find_each do |doc|
        hash = compute_hash(doc)
        next unless hash

        doc.update_columns(content_hash: hash) unless @dry_run
        @stats[:backfilled] += 1
      end
    end

    def find_and_merge_duplicates
      dupes = Document.where.not(content_hash: nil)
                      .group(:content_hash, :email_account_id)
                      .having("COUNT(*) > 1")
                      .pluck(:content_hash, :email_account_id, Arel.sql("ARRAY_AGG(id ORDER BY status ASC, ai_confidence_score DESC NULLS LAST, created_at ASC)"))

      Rails.logger.info("[Documents::DedupService] Found #{dupes.size} duplicate groups")
      @stats[:groups] = dupes.size

      dupes.each do |hash, account_id, ids|
        keep_id = ids.first
        merge_ids = ids[1..]

        merge_ids.each do |dup_id|
          merge_into(keep_id, dup_id)
        end

        @stats[:kept] += 1
        @stats[:merged] += merge_ids.size
      end
    end

    def merge_into(keep_id, duplicate_id)
      keep = Document.find(keep_id)
      dup = Document.find(duplicate_id)

      ActiveRecord::Base.transaction do
        # Move all email_message links from the duplicate to the kept document
        dup.document_email_messages.find_each do |dem|
          unless keep.document_email_messages.exists?(email_message_id: dem.email_message_id)
            if @dry_run
              Rails.logger.info("[DRY RUN] Would link document #{keep_id} to email_message #{dem.email_message_id}")
            else
              keep.document_email_messages.create!(email_message_id: dem.email_message_id)
            end
          end
        end

        # If the kept document doesn't have the legacy email_message_id but the duplicate does,
        # adopt it from the duplicate if more complete
        if !@dry_run
          # Adopt AI-extracted data from the duplicate if the kept one has less
          if keep.ai_extraction_data.blank? && dup.ai_extraction_data.present?
            keep.update_columns(
              ai_extraction_data: dup.ai_extraction_data,
              ai_confidence_score: dup.ai_confidence_score,
              metadata: dup.metadata,
              vendor_name: dup.vendor_name.presence || keep.vendor_name,
              client_name: dup.client_name.presence || keep.client_name,
              invoice_number: dup.invoice_number.presence || keep.invoice_number,
              receipt_number: dup.receipt_number.presence || keep.receipt_number,
              document_date: dup.document_date || keep.document_date,
              amount_cents: dup.amount_cents || keep.amount_cents,
              bank_name: dup.bank_name.presence || keep.bank_name
            )
          end

          # Delete the duplicate document
          dup.document_email_messages.delete_all
          dup.original_file.purge if dup.original_file.attached?
          dup.processed_pdf.purge if dup.processed_pdf.attached?
          dup.delete
        else
          Rails.logger.info("[DRY RUN] Would merge document #{duplicate_id} into #{keep_id} and delete #{duplicate_id}")
        end
      end
    end

    def compute_hash(doc)
      return nil unless doc.original_file.attached?

      doc.original_file.blob.download
      doc.original_file.blob.checksum
    rescue => e
      Rails.logger.warn("[Documents::DedupService] Could not hash document #{doc.id}: #{e.message}")
      nil
    end
  end
end
