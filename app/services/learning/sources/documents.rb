module Learning
  module Sources
    # Document-classification verdicts, read straight off the approved-documents
    # corpus ("the records ARE the memory" — there is no separate table). Built
    # around one subject document and excludes it from its own corpus. Targeted:
    # each tier is a live query, and the cascade short-circuits so the filename
    # scan only runs when the sender tier had no consensus.
    #
    # Carries the exact by_sender / by_filename logic that used to live inline in
    # Documents::ClassificationMemory.
    class Documents < Base
      # Bound the corpus we scan so a large workspace stays fast.
      CORPUS_LIMIT = 500

      def initialize(document)
        @document = document
        @workspace = document.workspace
      end

      # Sender consensus beats filename — who sent it is a stronger prior than
      # what it happens to be named.
      def signal_cascade = %i[sender filename]

      def tally_for(signal, **)
        labels =
          case signal
          when :sender then by_sender
          when :filename then by_filename
          else []
          end
        labels = labels.compact
        labels.empty? ? nil : labels.tally
      end

      private

      def approved_corpus
        @workspace.documents
                  .where(review_status: :approved)
                  .where.not(id: @document.id)
                  .where.not(document_type_id: nil)
      end

      # Dominant approved type among documents from the same sender (by name,
      # falling back to the email account when the document has no sender name).
      def by_sender
        sender = @document.sender_name.to_s.strip
        account_id = @document.email_account_id
        return [] if sender.blank? && account_id.blank?

        scope = approved_corpus.limit(CORPUS_LIMIT)
        scope = if sender.present?
          scope.where("LOWER(documents.metadata->>'sender_name') = ?", sender.downcase)
        else
          scope.where(email_account_id: account_id)
        end
        scope.pluck(:document_type_id)
      end

      # Dominant approved type among documents whose filename normalizes to the
      # same stem. The filename lives on the ActiveStorage blob, so we join to it
      # and normalize the bounded result set in Ruby.
      def by_filename
        return [] unless @document.original_file.attached?

        my_stem = Learning::Helpers::FilenameStem.call(@document.original_file.filename.to_s)
        return [] if my_stem.blank?

        rows = approved_corpus.joins(:original_file_blob)
                              .limit(CORPUS_LIMIT)
                              .pluck("active_storage_blobs.filename", :document_type_id)
        rows.filter_map { |fname, type_id| type_id if Learning::Helpers::FilenameStem.call(fname) == my_stem }
      end
    end
  end
end
