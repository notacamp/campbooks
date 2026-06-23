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
  class ClassificationMemory
    # A signal only counts when there are at least this many approved examples and the
    # dominant type holds at least this share of them — enough to trust, not noise.
    MIN_EXAMPLES = 3
    MIN_SHARE = 0.6
    # Bound the corpus we scan so a large workspace stays fast.
    CORPUS_LIMIT = 500

    Suggestion = Data.define(:document_type_id, :type_name, :source, :count, :total)

    def initialize(document)
      @document = document
      @workspace = document.workspace
    end

    # The strongest available signal, or nil. Sender consensus beats filename — who
    # sent it is a stronger prior than what it happens to be named.
    def suggestion
      return @suggestion if defined?(@suggestion)
      @suggestion = (by_sender || by_filename)
    end

    # A one-line natural-language hint to bias the AI prompt, or nil when we've not
    # seen enough approved examples to say anything useful.
    def prompt_hint
      s = suggestion
      return nil unless s

      origin = s.source == :sender ? "from this sender" : "with similar filenames"
      "Learned from past human-approved documents in this workspace: #{s.count} of " \
        "#{s.total} documents #{origin} were classified as \"#{s.type_name}\". Strongly " \
        "prefer \"#{s.type_name}\" unless the content clearly indicates a different type."
    end

    private

    def approved_corpus
      @workspace.documents.where(review_status: :approved).where.not(id: @document.id)
    end

    # Dominant approved type among documents from the same sender (by name, falling
    # back to the email account when the document has no sender name).
    def by_sender
      sender = @document.sender_name.to_s.strip
      account_id = @document.email_account_id
      return nil if sender.blank? && account_id.blank?

      scope = approved_corpus.where.not(document_type_id: nil).limit(CORPUS_LIMIT)
      scope = if sender.present?
        scope.where("LOWER(documents.sender_name) = ?", sender.downcase)
      else
        scope.where(email_account_id: account_id)
      end

      consensus(scope.pluck(:document_type_id), :sender)
    end

    # Dominant approved type among documents whose filename normalizes to the same
    # stem (digits/dates/separators stripped). The filename lives on the ActiveStorage
    # blob, so we join to it and normalize the bounded result set in Ruby.
    def by_filename
      return nil unless @document.original_file.attached?

      my_stem = filename_stem(@document.original_file.filename.to_s)
      return nil if my_stem.blank?

      rows = approved_corpus.where.not(document_type_id: nil)
                            .joins(:original_file_blob)
                            .limit(CORPUS_LIMIT)
                            .pluck("active_storage_blobs.filename", :document_type_id)

      matching = rows.filter_map { |fname, type_id| type_id if filename_stem(fname) == my_stem }
      consensus(matching, :filename)
    end

    def consensus(type_ids, source)
      type_ids = type_ids.compact
      return nil if type_ids.size < MIN_EXAMPLES

      top_id, count = type_ids.tally.max_by { |_, c| c }
      return nil if (count.to_f / type_ids.size) < MIN_SHARE

      name = DocumentType.where(id: top_id).pick(:name)
      return nil if name.blank?

      Suggestion.new(document_type_id: top_id, type_name: name, source: source,
                     count: count, total: type_ids.size)
    end

    # Normalize a filename to a token-set "stem": downcase, drop the extension, strip
    # digits (dates, invoice/policy numbers) and separators, drop short tokens, then
    # sort+dedupe so order and numbering don't matter. e.g. "Fatura_EDP_2026-01.pdf"
    # and "fatura-edp-2026-02.pdf" both → "edp fatura".
    def filename_stem(name)
      base = name.to_s.downcase
      base = base.sub(/\.[a-z0-9]{1,5}\z/, "")
      base = base.gsub(/[0-9]+/, " ").gsub(/[^a-z]+/, " ")
      base.split.select { |t| t.length >= 3 }.uniq.sort.join(" ").presence
    end
  end
end
