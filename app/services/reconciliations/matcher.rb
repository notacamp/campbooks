# frozen_string_literal: true

module Reconciliations
  # Matches each unmatched BankTransaction in a Reconciliation against workspace
  # Documents using heuristics and (optionally) AI disambiguation.
  # include ActionView::RecordIdentifier so dom_id() works without a view context.
  #
  # Scoring (0..1):
  #   amount:   exact cents match → 0.5; within 2% → 0.35
  #   date:     d = min days from booked_on to document_date / due_date
  #             0.25 at d≤1, linearly decays to 0 at d=30
  #   name:     Jaccard over word tokens; if ≥0.4 → adds 0.25 × similarity
  #
  # Auto-suggests when top score ≥ 0.85 (up to 3 matches).
  # Falls back to AI disambiguation when candidates exist and text AI is configured.
  # Falls back to top heuristic when 0.5 ≤ top < 0.85 and AI unavailable.
  #
  # Broadcasts a per-row turbo_stream replacement for every changed transaction.
  # Also broadcasts an updated summary bar every 10 changes and at the end.
  class Matcher
    include ActionView::RecordIdentifier # dom_id(record) / dom_id(record, :card)

    SCORE_AUTO_SUGGEST = 0.85
    SCORE_AI_THRESHOLD = 0.6
    SCORE_HEURISTIC_FALLBACK = 0.5
    AI_MAX_CANDIDATES = 10
    MAX_SUGGESTIONS = 3
    AMOUNT_CLOSE_TOLERANCE = 0.02  # 2% — same tolerance the heuristic score uses
    AI_CONFIDENCE_CAP_CLOSE = 0.85 # AI conf ceiling when amounts are close, not exact
    AI_CONFIDENCE_CAP_SPLIT = 0.8  # per-doc ceiling for split payments (docs sum to txn)
    DATE_DECAY_DAYS = 30

    # @param reconciliation [Reconciliation]
    # @param workspace [Workspace]
    def initialize(reconciliation:, workspace:)
      @reconciliation = reconciliation
      @workspace      = workspace
      @change_count   = 0
    end

    def call
      # Fix 13a: Prefetch document_ids that are already confirmed in OTHER reconciliations
      # in this workspace — avoids N×M EXISTS queries inside cross_reconciliation_warning?.
      @cross_recon_doc_ids = Set.new(
        TransactionMatch.joins(:bank_transaction)
                        .where(status: :confirmed)
                        .where.not(bank_transactions: { reconciliation_id: @reconciliation.id })
                        .where(document_id: @workspace.documents.select(:id))
                        .distinct
                        .pluck(:document_id)
      )

      transactions = @reconciliation.bank_transactions.ordered
                                    .where(status: :unmatched)

      transactions.each do |txn|
        match_transaction(txn)
      end

      broadcast_summary_bar
    end

    private

    def match_transaction(txn)
      candidates = candidate_pool(txn)

      scored = candidates.map { |doc| [ doc, score(txn, doc) ] }
                         .sort_by { |_, s| -s }

      top_score = scored.first&.last || 0.0

      if top_score >= SCORE_AUTO_SUGGEST
        create_suggested_matches(txn, scored.first(MAX_SUGGESTIONS), method: :heuristic)
      elsif candidates.any? && ai_text_configured?
        # If AI produced no suggestions (no config rows, error, or empty result),
        # fall through to the heuristic fallback so self-hosted installs still
        # get a suggestion when the score is ≥ SCORE_HEURISTIC_FALLBACK.
        ai_suggested = ai_match(txn, scored.first(AI_MAX_CANDIDATES))
        unless ai_suggested
          create_suggested_matches(txn, scored.first(1), method: :heuristic) if top_score >= SCORE_HEURISTIC_FALLBACK
        end
      elsif top_score >= SCORE_HEURISTIC_FALLBACK
        create_suggested_matches(txn, scored.first(1), method: :heuristic)
      end
      # else: stays unmatched, no broadcast needed
    rescue *Ai::Adapters::Base::TRANSIENT_ERRORS
      raise # let MatchJob's retry_on handle 429/5xx
    rescue => e
      Rails.logger.error("[Reconciliations::Matcher] txn #{txn.id} failed: #{e.class}: #{e.message}")
    end

    # ── Candidate pool ──────────────────────────────────────────────────────────

    def candidate_pool(txn)
      base = @workspace.documents
                       .where(document_type: txn.candidate_document_types)
                       .where.not(amount_cents: [ nil, 0 ])

      # Currency equality — normalize both sides to ISO codes.
      txn_currency = normalize_currency(txn.currency.to_s)
      # We filter in Ruby after fetching to avoid complex SQL on a normalized value.

      # Date window: document_date within 90d before / 15d after booked_on,
      # OR due_date within ±30d of booked_on.
      booked = txn.booked_on
      doc_date_range = ((booked - 90.days)..(booked + 15.days))
      due_date_range = ((booked - 30.days)..(booked + 30.days))

      docs = base.where(
        "(document_date BETWEEN :dstart AND :dend) OR (due_date BETWEEN :dustart AND :duend)",
        dstart:  doc_date_range.begin,  dend:  doc_date_range.end,
        dustart: due_date_range.begin, duend: due_date_range.end
      ).limit(50).to_a

      # Filter by normalized currency match.
      docs.select { |doc| normalize_currency(doc.currency.to_s) == txn_currency }
    end

    # ── Scoring ─────────────────────────────────────────────────────────────────

    def score(txn, doc)
      amount_score(txn, doc) + date_score(txn, doc) + name_score(txn, doc)
    end

    def amount_score(txn, doc)
      txn_abs = txn.amount_cents.abs
      doc_abs = doc.amount_cents.abs
      return 0.0 if txn_abs.zero? || doc_abs.zero?

      if txn_abs == doc_abs
        0.5
      elsif (txn_abs - doc_abs).abs.to_f / [ txn_abs, doc_abs ].max <= 0.02
        0.35
      else
        0.0
      end
    end

    def date_score(txn, doc)
      dates = [ doc.document_date, doc.due_date ].compact
      return 0.0 if dates.empty?

      min_delta = dates.map { |d| (txn.booked_on - d).abs }.min
      return 0.25 if min_delta <= 1
      return 0.0  if min_delta >= DATE_DECAY_DAYS

      0.25 * (1.0 - (min_delta.to_f - 1) / (DATE_DECAY_DAYS - 1))
    end

    def name_score(txn, doc)
      # Fix 13b: delegate to name_jaccard so both this and build_reasons share the cache.
      sim = name_jaccard(txn, doc)
      sim >= 0.4 ? 0.25 * sim : 0.0
    end

    # Jaccard similarity on two sets of word tokens.
    def jaccard_similarity(set_a, set_b)
      return 0.0 if set_a.empty? || set_b.empty?

      intersection = (set_a & set_b).size.to_f
      union        = (set_a | set_b).size.to_f
      union.zero? ? 0.0 : intersection / union
    end

    # Normalize text to word tokens: downcase, strip diacritics, split on non-alnum.
    def tokenize(text)
      normalized = text.unicode_normalize(:nfkd)
                       .encode("ASCII", invalid: :replace, undef: :replace, replace: "")
                       .downcase
      normalized.split(/[^a-z0-9]+/).reject(&:empty?).to_set
    end

    # Normalize currency codes from symbols and full names to ISO-4217.
    CURRENCY_MAP = {
      "€"          => "EUR", "euro"       => "EUR", "euros"      => "EUR",
      "eur"        => "EUR",
      "$"          => "USD", "us dollar"  => "USD", "usd"        => "USD",
      "us dollars" => "USD",
      "£"          => "GBP", "gbp"        => "GBP", "pound"      => "GBP",
      "pounds"     => "GBP",
      "r$"         => "BRL", "brl"        => "BRL", "real"       => "BRL",
      "¥"          => "JPY", "jpy"        => "JPY", "yen"        => "JPY",
      "chf"        => "CHF", "franc"      => "CHF"
    }.freeze

    def normalize_currency(raw)
      stripped = raw.strip.downcase
      CURRENCY_MAP[stripped] || stripped.upcase
    end

    # ── Match creation ──────────────────────────────────────────────────────────

    # Fix 13a: Uses the prefetched Set from call() — O(1) lookup instead of EXISTS query.
    def cross_reconciliation_warning?(doc)
      @cross_recon_doc_ids&.include?(doc.id) || false
    end

    def create_suggested_matches(txn, scored_docs, method:)
      txn_changed = false
      scored_docs.each do |doc, score_val|
        reasons = build_reasons(txn, doc, score_val)
        match = TransactionMatch.find_or_initialize_by(
          bank_transaction_id: txn.id,
          document_id:         doc.id
        )
        next if match.confirmed?

        match.assign_attributes(
          status:       :suggested,
          matched_by:   method,
          confidence:   score_val.round(4),
          match_reasons: reasons
        )
        match.save! if match.changed?
        txn_changed = true
      end

      if txn_changed
        txn.update!(status: :suggested) unless txn.suggested?
        broadcast_row(txn)
        @change_count += 1
        broadcast_summary_bar if (@change_count % 10).zero?
      end
    end

    # Bounds AI-claimed confidence by objective evidence. Amount is the anchor:
    #   exact               → keep the model's confidence as-is
    #   within 2%           → cap at AI_CONFIDENCE_CAP_CLOSE
    #   split payment       → several returned docs whose |amounts| SUM to the
    #                         transaction (±2%) are all kept, capped per doc
    #   otherwise           → discard the match entirely (log it)
    # Also collapses near-duplicate documents (same party, amount and date —
    # e.g. a receipt and its invoice) into the single best candidate.
    # Returns an array of [match_hash, document] pairs.
    def ground_ai_matches(txn, matches, doc_map)
      pairs = matches.filter_map do |m|
        doc = doc_map[m["document_id"]]
        doc ? [ m, doc ] : nil # unknown/hallucinated ids never match
      end
      return [] if pairs.empty?

      pairs = dedupe_twin_documents(pairs)

      txn_abs = txn.amount_cents.abs
      if pairs.size > 1
        sum = pairs.sum { |_, doc| doc.amount_cents.to_i.abs }
        if close_amounts?(sum, txn_abs)
          return pairs.map { |m, doc|
            [ m.merge("confidence" => [ m["confidence"].to_f, AI_CONFIDENCE_CAP_SPLIT ].min), doc ]
          }
        end
      end

      pairs.filter_map do |m, doc|
        doc_abs = doc.amount_cents.to_i.abs
        if doc_abs == txn_abs
          [ m, doc ]
        elsif close_amounts?(doc_abs, txn_abs)
          [ m.merge("confidence" => [ m["confidence"].to_f, AI_CONFIDENCE_CAP_CLOSE ].min), doc ]
        else
          Rails.logger.info(
            "[Reconciliations::Matcher] discarded AI match for txn #{txn.id}: " \
            "doc #{doc.id} amount #{doc_abs} vs txn #{txn_abs} (claimed conf #{m["confidence"]})"
          )
          nil
        end
      end
    end

    def close_amounts?(a, b)
      return false if a.zero? || b.zero?

      (a - b).abs.to_f / [ a, b ].max <= AMOUNT_CLOSE_TOLERANCE
    end

    # A receipt and its invoice for the same purchase share party, amount and
    # date — suggesting both reads as two different matches. Keep the best one
    # (prefer a document with an invoice number, then the model's confidence).
    def dedupe_twin_documents(pairs)
      pairs.group_by { |_, doc|
        [ (doc.vendor_name.presence || doc.client_name).to_s.downcase.strip,
          doc.amount_cents, doc.document_date ]
      }.values.map { |group|
        group.max_by { |m, doc| [ doc.invoice_number.present? ? 1 : 0, m["confidence"].to_f ] }
      }
    end

    def build_reasons(txn, doc, score_val)
      reasons = {}
      # Amount
      txn_abs = txn.amount_cents.abs
      doc_abs = doc.amount_cents.abs
      if txn_abs == doc_abs
        reasons["amount"] = "exact"
      elsif (txn_abs - doc_abs).abs.to_f / [ txn_abs, doc_abs ].max <= 0.02
        reasons["amount"] = "close"
      end
      # Date
      dates = [ doc.document_date, doc.due_date ].compact
      if dates.any?
        delta = dates.map { |d| (txn.booked_on - d).abs }.min
        reasons["date_delta_days"] = delta.to_i
      end
      # Name similarity
      sim = name_jaccard(txn, doc)
      reasons["name_similarity"] = sim.round(2) if sim > 0
      # Cross-reconciliation
      if cross_reconciliation_warning?(doc)
        reasons["cross_reconciliation_warning"] = true
      end
      reasons
    end

    # Fix 13b: memoize per (txn, doc) pair — score() and build_reasons() both call
    # this, so without the cache we'd compute Jaccard twice per candidate.
    def name_jaccard(txn, doc)
      @jaccard_cache ||= {}
      key = [ txn.id, doc.id ]
      return @jaccard_cache[key] if @jaccard_cache.key?(key)

      txn_text = (txn.counterparty.presence || txn.description.to_s).downcase
      doc_text  = (doc.vendor_name.presence || doc.client_name.to_s).downcase

      sim = if txn_text.blank? || doc_text.blank?
        0.0
      else
        jaccard_similarity(tokenize(txn_text), tokenize(doc_text))
      end

      @jaccard_cache[key] = sim
    end

    # ── AI disambiguation ────────────────────────────────────────────────────────

    def ai_text_configured?
      Ai::ProviderSetup.configured?(@workspace, :text)
    end

    def ai_match(txn, scored_candidates)
      config = Ai::Configuration.for_any(Ai::ReminderExtractor::PURPOSES)
      return nil unless config

      candidates = scored_candidates.first(AI_MAX_CANDIDATES).map.with_index(1) do |(doc, _), idx|
        <<~CANDIDATE
          #{idx}. [id: #{doc.id}] #{doc.classification&.name || doc.document_type} — #{doc.document_date&.iso8601 || "no date"} — #{doc.amount_cents&.then { |c| format("%.2f", c / 100.0) } || "?"} #{doc.currency} — #{doc.vendor_name.presence || doc.client_name.presence || "unknown"} #{doc.invoice_number.present? ? "(inv. #{doc.invoice_number})" : ""}
        CANDIDATE
      end.join("\n")

      prompt = <<~PROMPT
        Match this bank transaction to the best document(s) from the list.

        Transaction:
        - Date: #{txn.booked_on.iso8601}
        - Description: #{txn.description}
        - Amount: #{format("%.2f", txn.amount_cents / 100.0)} #{txn.currency} (#{txn.debit? ? "debit/payment" : "credit/receipt"})
        - Counterparty: #{txn.counterparty.presence || "unknown"}

        Documents (by number):
        #{candidates}

        Respond ONLY with valid JSON:
        {"matches": [{"document_id": "<uuid>", "confidence": 0.0-1.0, "reason": "one sentence"}]}

        Include only matches with confidence ≥ #{SCORE_AI_THRESHOLD}. Return [] if none are good matches.
      PROMPT

      text = config[:adapter].chat(
        system:      "You are a financial document matching assistant. Always respond with valid JSON only.",
        messages:    [ { role: "user", content: prompt } ],
        model:       config[:model],
        max_tokens:  1000,
        temperature: 0.0
      )

      parsed = Ai::ChatService.parse_json_response(text, object_start: /\{\s*"matches"/)
      matches = Array(parsed["matches"]).select { |m| m["confidence"].to_f >= SCORE_AI_THRESHOLD }
      return nil if matches.empty? # signal no suggestion to match_transaction

      doc_map = scored_candidates.first(AI_MAX_CANDIDATES).map { |(doc, _)| [ doc.id, doc ] }.to_h

      # The model's confidence is a claim, not evidence: bound it by objective
      # signals before anything reaches the user (prod incident: a €69.99
      # invoice suggested against an €85.98 payment at 0.95 with a hallucinated
      # vendor relationship).
      grounded = ground_ai_matches(txn, matches, doc_map)
      return nil if grounded.empty?

      txn_changed = false
      grounded.first(MAX_SUGGESTIONS).each do |m, doc|
        reasons = build_reasons(txn, doc, m["confidence"].to_f)
        reasons["ai_reason"] = m["reason"] if m["reason"].present?

        match = TransactionMatch.find_or_initialize_by(
          bank_transaction_id: txn.id,
          document_id:         doc.id
        )
        next if match.confirmed?

        match.assign_attributes(
          status:        :suggested,
          matched_by:    :ai,
          confidence:    m["confidence"].to_f.round(4),
          match_reasons: reasons
        )
        match.save! if match.changed?
        txn_changed = true
      end

      if txn_changed
        txn.update!(status: :suggested) unless txn.suggested?
        broadcast_row(txn)
        @change_count += 1
        broadcast_summary_bar if (@change_count % 10).zero?
      end

      txn_changed # return truthy when suggestions were created, nil/false otherwise
    rescue *Ai::Adapters::Base::TRANSIENT_ERRORS
      raise
    rescue => e
      Rails.logger.warn("[Reconciliations::Matcher] AI disambiguation failed for txn #{txn.id}: #{e.class}: #{e.message}")
      # Degrade gracefully — never fail the whole job for one transaction's AI call.
      nil # indicate AI produced no usable suggestion
    end

    # ── Broadcasting ─────────────────────────────────────────────────────────────

    def broadcast_row(txn)
      txn_fresh = txn.reload.tap { |t| t.association(:transaction_matches).reset }
      txn_fresh.transaction_matches.reload

      locale = @reconciliation.created_by&.locale.presence || I18n.default_locale
      I18n.with_locale(locale) do
        row_html  = ApplicationController.render(
          partial: "bank_transactions/row",
          locals:  { transaction: txn_fresh, reconciliation: @reconciliation },
          layout:  false
        )
        card_html = ApplicationController.render(
          partial: "bank_transactions/card",
          locals:  { transaction: txn_fresh, reconciliation: @reconciliation },
          layout:  false
        )

        Turbo::StreamsChannel.broadcast_replace_to(
          "reconciliation_#{@reconciliation.id}",
          target: dom_id(txn_fresh),
          html:   row_html
        )
        Turbo::StreamsChannel.broadcast_replace_to(
          "reconciliation_#{@reconciliation.id}",
          target: dom_id(txn_fresh, :card),
          html:   card_html
        )
      end
    rescue => e
      Rails.logger.warn("[Reconciliations::Matcher] broadcast_row failed: #{e.class}: #{e.message}")
    end

    def broadcast_summary_bar
      locale = @reconciliation.created_by&.locale.presence || I18n.default_locale
      I18n.with_locale(locale) do
        counts  = @reconciliation.bank_transactions.group(:status).count
        nif_count = nif_exception_count
        html = ApplicationController.render(
          partial: "reconciliations/summary_bar",
          locals:  { reconciliation: @reconciliation, status_counts: counts, nif_exception_count: nif_count },
          layout:  false
        )
        Turbo::StreamsChannel.broadcast_replace_to(
          "reconciliation_#{@reconciliation.id}",
          target: "reconciliation_summary_bar",
          html:   html
        )
      end
    rescue => e
      Rails.logger.warn("[Reconciliations::Matcher] broadcast_summary_bar failed: #{e.class}: #{e.message}")
    end

    def nif_exception_count
      @reconciliation.nif_exception_count(@workspace.company_nif.presence)
    end
  end
end
