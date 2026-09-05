# frozen_string_literal: true

module Reconciliations
  # Query object: given a single BankTransaction, finds and scores the closest
  # unlinked documents from the workspace's archive using the same heuristics
  # as Reconciliations::Matcher (amount + date proximity + name similarity).
  #
  # Extracted from Matcher so both the matching engine and the hunt panel's
  # near-miss UI share a single scoring implementation.
  #
  # Usage:
  #   cands = Reconciliations::Candidates.new(
  #             bank_transaction:    txn,
  #             workspace:           ws,
  #             cross_recon_doc_ids: already_matched_set  # optional Set
  #           )
  #   cands.call                     # → [{document:, score:, reasons:}] sorted desc
  #   cands.below_threshold          # → same, filtered to 0 < score < SCORE_AUTO_SUGGEST
  #   cands.pool                     # → [Document] (raw unscored candidate pool)
  class Candidates
    SCORE_AUTO_SUGGEST     = Reconciliations::Matcher::SCORE_AUTO_SUGGEST
    AMOUNT_CLOSE_TOLERANCE = Reconciliations::Matcher::AMOUNT_CLOSE_TOLERANCE
    NAME_AGREEMENT_THRESHOLD = Reconciliations::Matcher::NAME_AGREEMENT_THRESHOLD
    DATE_DECAY_DAYS        = Reconciliations::Matcher::DATE_DECAY_DAYS

    # Currency symbol/name → ISO-4217 code — kept in sync with Matcher::CURRENCY_MAP
    CURRENCY_MAP = Reconciliations::Matcher::CURRENCY_MAP

    def initialize(bank_transaction:, workspace:, cross_recon_doc_ids: nil)
      @txn                  = bank_transaction
      @workspace            = workspace
      @cross_recon_doc_ids  = cross_recon_doc_ids  # nil = skip cross-recon warning
    end

    # Returns Array<Hash> sorted by score descending.
    # Each element: { document: Document, score: Float, reasons: Hash }
    def call
      @scored ||= pool.map do |doc|
        sc = score_doc(doc)
        { document: doc, score: sc, reasons: build_reasons(doc, sc) }
      end.sort_by { |h| -h[:score] }
    end

    # Candidates with 0 < score < SCORE_AUTO_SUGGEST — the matcher ignored them
    # (too low to auto-suggest), but they are still relevant for the hunt panel.
    def below_threshold
      call.select { |h| h[:score] > 0 && h[:score] < SCORE_AUTO_SUGGEST }
    end

    # Raw document pool (unscored) — used by Matcher for the AI disambiguation path.
    def pool
      @pool ||= begin
        base = @workspace.documents
                         .where(document_type: @txn.candidate_document_types)
                         .where(
                           "documents.metadata->>'amount_cents' IS NOT NULL AND " \
                           "(CASE WHEN documents.metadata->>'amount_cents' ~ ? " \
                           "THEN (documents.metadata->>'amount_cents')::bigint END) <> 0",
                           Document::AMOUNT_CENTS_REGEX
                         )

        txn_currency = normalize_currency(@txn.currency.to_s)

        booked = @txn.booked_on
        doc_date_range = ((booked - 90.days)..(booked + 15.days))
        due_date_range = ((booked - 30.days)..(booked + 30.days))

        docs = base.where(
          "(documents.metadata->>'document_date' >= :dstart AND documents.metadata->>'document_date' <= :dend) " \
          "OR (documents.metadata->>'due_date' >= :dustart AND documents.metadata->>'due_date' <= :duend)",
          dstart:  doc_date_range.begin.to_s, dend:  doc_date_range.end.to_s,
          dustart: due_date_range.begin.to_s, duend: due_date_range.end.to_s
        ).limit(50).to_a

        docs.select { |doc| normalize_currency(doc.currency.to_s) == txn_currency }
      end
    end

    # ── Scoring ──────────────────────────────────────────────────────────────

    def score_doc(doc)
      amount_score(doc) + date_score(doc) + name_score(doc)
    end

    def build_reasons(doc, score_val)
      reasons = {}
      txn_abs = @txn.amount_cents.abs
      doc_abs = doc.amount_cents.abs

      if txn_abs == doc_abs
        reasons["amount"] = "exact"
      elsif close_amounts?(txn_abs, doc_abs)
        reasons["amount"] = "close"
      end

      dates = [ doc.document_date, doc.due_date ].compact
      if dates.any?
        delta = dates.map { |d| (@txn.booked_on - d).abs }.min
        reasons["date_delta_days"] = delta.to_i
      end

      sim = name_jaccard(doc)
      reasons["name_similarity"] = sim.round(2) if sim > 0

      if @cross_recon_doc_ids&.include?(doc.id)
        reasons["cross_reconciliation_warning"] = true
      end

      reasons
    end

    private

    # ── Amount scoring ───────────────────────────────────────────────────────

    def amount_score(doc)
      txn_abs = @txn.amount_cents.abs
      doc_abs = doc.amount_cents.abs
      return 0.0 if txn_abs.zero? || doc_abs.zero?

      if txn_abs == doc_abs
        0.5
      elsif close_amounts?(txn_abs, doc_abs)
        0.35
      else
        0.0
      end
    end

    def date_score(doc)
      dates = [ doc.document_date, doc.due_date ].compact
      return 0.0 if dates.empty?

      min_delta = dates.map { |d| (@txn.booked_on - d).abs }.min
      return 0.25 if min_delta <= 1
      return 0.0  if min_delta >= DATE_DECAY_DAYS

      0.25 * (1.0 - (min_delta.to_f - 1) / (DATE_DECAY_DAYS - 1))
    end

    def name_score(doc)
      sim = name_jaccard(doc)
      sim >= NAME_AGREEMENT_THRESHOLD ? 0.25 * sim : 0.0
    end

    def close_amounts?(a, b)
      return false if a.zero? || b.zero?

      (a - b).abs.to_f / [ a, b ].max <= AMOUNT_CLOSE_TOLERANCE
    end

    # Memoized Jaccard similarity between the transaction's name tokens and a
    # document's vendor/client tokens. Cache key is the document id so repeated
    # calls within the same instance are O(1) after the first computation.
    def name_jaccard(doc)
      @jaccard_cache ||= {}
      return @jaccard_cache[doc.id] if @jaccard_cache.key?(doc.id)

      txn_text = (@txn.counterparty.presence || @txn.description.to_s).downcase
      doc_text  = (doc.vendor_name.presence || doc.client_name.to_s).downcase

      sim = if txn_text.blank? || doc_text.blank?
        0.0
      else
        jaccard_similarity(tokenize(txn_text), tokenize(doc_text))
      end

      @jaccard_cache[doc.id] = sim
    end

    def jaccard_similarity(set_a, set_b)
      return 0.0 if set_a.empty? || set_b.empty?

      intersection = (set_a & set_b).size.to_f
      union        = (set_a | set_b).size.to_f
      union.zero? ? 0.0 : intersection / union
    end

    def tokenize(text)
      normalized = text.unicode_normalize(:nfkd)
                       .encode("ASCII", invalid: :replace, undef: :replace, replace: "")
                       .downcase
      normalized.split(/[^a-z0-9]+/).reject(&:empty?).to_set
    end

    def normalize_currency(raw)
      stripped = raw.strip.downcase
      CURRENCY_MAP[stripped] || stripped.upcase
    end
  end
end
