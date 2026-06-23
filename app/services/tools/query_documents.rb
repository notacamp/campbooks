module Tools
  class QueryDocuments
    def self.call(args = {})
      limit = [ args["limit"].to_i, 50 ].min
      limit = 20 if limit <= 0

      # Documents are workspace-scoped; fail closed if no workspace is established.
      return { count: 0, documents: [], search_method: "none" } unless Current.workspace

      # Try semantic search if search_text is provided and embeddings exist
      if args["search_text"].present? && SearchRecord.by_type("Document").exists?
        filters = build_filters(args)
        results = SearchService.search(
          args["search_text"],
          workspace: Current.workspace,
          filters: filters.merge(searchable_type: "Document"),
          options: { limit: limit }
        )

        if results.any?
          document_ids = results.map(&:searchable_id)
          documents = Current.workspace.documents.where(id: document_ids)
                              .order(document_date: :desc)
                              .limit(limit)
                              .map { |doc| format_document(doc) }

          return { count: documents.size, documents: documents, search_method: "semantic" }
        end
      end

      # Fallback to ILIKE search
      scope = Current.workspace.documents
      scope = apply_filters(scope, args)

      if args["search_text"].present?
        text = "%#{args["search_text"]}%"
        scope = scope.where("vendor_name ILIKE ? OR description ILIKE ? OR invoice_number ILIKE ? OR client_name ILIKE ?", text, text, text, text)
      end

      total = scope.count
      documents = scope.order(document_date: :desc).limit(limit).map { |doc| format_document(doc) }

      { count: total, documents: documents, search_method: "text" }
    end

    def self.apply_filters(scope, args)
      # "status" is the legacy single-axis arg — treat it as the human review status.
      review = args["review_status"] || args["status"]
      scope = scope.where(review_status: review) if review.present?
      scope = scope.where(ai_status: args["ai_status"]) if args["ai_status"].present?
      scope = scope.where(document_type: args["document_type"]) if args["document_type"].present?
      scope = scope.where("vendor_name ILIKE ?", "%#{args["vendor_name"]}%") if args["vendor_name"].present?
      scope = scope.where("amount_cents >= ?", args["amount_min_cents"].to_i) if args["amount_min_cents"].present?
      scope = scope.where("amount_cents <= ?", args["amount_max_cents"].to_i) if args["amount_max_cents"].present?

      if args["date_from"].present?
        date = Date.parse(args["date_from"]) rescue nil
        scope = scope.where("document_date >= ?", date) if date
      end

      if args["date_to"].present?
        date = Date.parse(args["date_to"]) rescue nil
        scope = scope.where("document_date <= ?", date) if date
      end

      scope = scope.where(source: args["source"]) if args["source"].present?
      scope
    end

    def self.build_filters(args)
      filters = {}
      review = args["review_status"] || args["status"]
      filters[:review_status] = review if review.present?
      filters[:ai_status] = args["ai_status"] if args["ai_status"].present?
      filters[:document_type] = args["document_type"] if args["document_type"].present?
      filters[:date_from] = args["date_from"] if args["date_from"].present?
      filters[:date_to] = args["date_to"] if args["date_to"].present?
      filters
    end

    def self.format_document(doc)
      {
        id: doc.id,
        document_type: Document.document_types.key(doc.document_type),
        vendor_name: doc.vendor_name,
        client_name: doc.client_name,
        amount_cents: doc.amount_cents,
        currency: doc.currency,
        ai_status: doc.ai_status,
        review_status: doc.review_status,
        document_date: doc.document_date&.iso8601,
        source: Document.sources.key(doc.source),
        invoice_number: doc.invoice_number
      }
    end
  end
end
