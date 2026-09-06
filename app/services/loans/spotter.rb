# frozen_string_literal: true

module Loans
  # Scout spots potential loan instalments: regular monthly debits with no invoice
  # and bank-ish counterparties or loan keywords.
  #
  # Returns up to 2 Suggestion structs based on pattern analysis of the workspace's
  # recent unmatched/excluded bank transactions.
  #
  # Cheap enough to run on every Money page load; results are memoized in the request.
  class Spotter
    Suggestion = Struct.new(
      :lender_guess,
      :source_counterparty,
      :instalment_cents,
      :currency,
      :first_seen_on,
      :last_seen_on,
      :count,
      :day_of_month,
      :previous_instalment_cents,  # set when a rate step was detected
      :sample_transaction_ids,
      :key,
      keyword_init: true
    )

    LOAN_KEYWORDS = Loans::Matcher::LOAN_KEYWORDS
    BANKISH = /\b(banco|bank|banque|bcp|millennium|caixa|cgd|santander|novo\s*banco|bpi|bankinter|abanca|montepio|eurobic|bnp|ing|revolut|n26|wise|credit|cr[eé]dito|bbva|sabadell|unicaja|soci[eé]t[eé]\s*g[eé]n[eé]rale|cr[eé]dit\s*agricole|lcl)\b/i

    MAX_SUGGESTIONS = 2
    CADENCE_MIN_DAYS = 25
    CADENCE_MAX_DAYS = 35
    MIN_COUNT = 2
    MERGE_AMOUNT_TOLERANCE = 0.05  # 5% for rate-reset merging
    COVERAGE_RECONCILIATIONS = 24

    def initialize(workspace)
      @workspace = workspace
    end

    def call
      @result ||= compute_suggestions
    end

    private

    def compute_suggestions
      txns = candidate_transactions
      return [] if txns.empty?

      # Group by [normalized counterparty, amount rounded to nearest euro]
      groups = txns.group_by do |txn|
        [ normalized_counterparty(txn), (txn.amount_cents.abs / 100.0).round * 100 ]
      end

      # Convert to candidates
      candidates = groups.filter_map { |(cp, cents), group_txns| build_candidate(cp, cents, group_txns) }

      # Merge groups where a rate reset happened (same counterparty, amounts within 5%, dates interleave)
      candidates = merge_on_rate_reset(candidates)

      # Exclude already-tracked and dismissed
      dismissed_keys  = dismissed_suggestion_keys
      tracked_cp_tokens, tracked_amounts = tracked_loan_data

      candidates.reject! do |c|
        dismissed_keys.include?(c[:key]) ||
          already_tracked?(c, tracked_cp_tokens, tracked_amounts)
      end

      candidates.first(MAX_SUGGESTIONS).map { |c| to_suggestion(c) }
    end

    def candidate_transactions
      recent_recs = @workspace.reconciliations.where(status: :ready).order(created_at: :desc).limit(COVERAGE_RECONCILIATIONS)
      return [] if recent_recs.empty?

      linked_ids = LoanInstalment.where.not(bank_transaction_id: nil).pluck(:bank_transaction_id)

      txns = BankTransaction
        .where(reconciliation_id: recent_recs.map(&:id))
        .where(status: [ BankTransaction.statuses[:unmatched], BankTransaction.statuses[:excluded] ])
        .where("amount_cents < 0")
        .to_a

      linked_set = linked_ids.to_set
      txns.reject { |t| linked_set.include?(t.id) }
    rescue StandardError => e
      Rails.logger.warn("[Loans::Spotter] candidate_transactions failed: #{e.class}: #{e.message}")
      []
    end

    def build_candidate(cp, rounded_cents, txns)
      return nil if txns.size < MIN_COUNT

      sorted = txns.sort_by(&:booked_on)

      # Check cadence: every gap between consecutive booked_on must be 25-35 days
      gaps = sorted.each_cons(2).map { |a, b| (b.booked_on - a.booked_on).to_i }
      return nil unless gaps.all? { |g| g.between?(CADENCE_MIN_DAYS, CADENCE_MAX_DAYS) }

      # Qualifies if any line matches LOAN_KEYWORDS OR counterparty matches BANKISH
      qualifies = txns.any? do |txn|
        text = "#{txn.description} #{txn.counterparty}"
        LOAN_KEYWORDS.match?(text) || BANKISH.match?(text)
      end
      return nil unless qualifies

      day = sorted.map { |t| t.booked_on.day }.group_by(&:itself).max_by { |_, v| v.size }&.first

      {
        counterparty:  cp,
        amount_cents:  rounded_cents,
        currency:      txns.first.currency,
        first_seen_on: sorted.first.booked_on,
        last_seen_on:  sorted.last.booked_on,
        count:         txns.size,
        day_of_month:  day,
        sample_ids:    sorted.last(3).map(&:id),
        previous_amount_cents: nil,
        key:           "#{cp}|#{rounded_cents}"
      }
    end

    def merge_on_rate_reset(candidates)
      merged = []
      used   = Set.new

      candidates.each_with_index do |a, i|
        next if used.include?(i)

        candidates.each_with_index do |b, j|
          next if i == j || used.include?(j)
          next unless a[:counterparty] == b[:counterparty]
          next unless a[:currency] == b[:currency]

          diff = (a[:amount_cents] - b[:amount_cents]).abs.to_f / [ a[:amount_cents], b[:amount_cents] ].max
          next unless diff <= MERGE_AMOUNT_TOLERANCE

          # Keep the larger (newer) amount as current; smaller is previous
          newer, older = [ a, b ].sort_by { |c| c[:amount_cents] }.reverse
          newer[:previous_amount_cents] = older[:amount_cents]
          newer[:first_seen_on]         = [ a[:first_seen_on], b[:first_seen_on] ].min
          newer[:count]                += older[:count]
          newer[:key]                   = "#{newer[:counterparty]}|#{newer[:amount_cents]}"

          merged << newer
          used << i
          used << j
        end

        merged << a unless used.include?(i)
        used << i
      end

      merged
    end

    def normalized_counterparty(txn)
      cp = txn.counterparty.presence

      if cp.present?
        cp.downcase.gsub(/[^a-z0-9 ]/i, " ").squeeze(" ").strip
      else
        # Fall back to first two distinctive description tokens
        tokens = txn.description.downcase.split(/\W+/)
                    .reject { |t| t.length < 3 }
                    .first(2)
        tokens.join(" ")
      end
    end

    def dismissed_suggestion_keys
      Array(@workspace.settings.fetch("dismissed_loan_suggestions", []))
    end

    def tracked_loan_data
      active_loans = @workspace.loans.active_loans
      tokens = active_loans.flat_map { |l| counterparty_tokens(l.effective_counterparty) }.to_set
      amounts = active_loans.map(&:instalment_cents).to_set
      [ tokens, amounts ]
    end

    def counterparty_tokens(name)
      name.to_s.downcase.split(/\W+/).reject { |t| t.length < 3 }
    end

    def already_tracked?(candidate, tracked_tokens, tracked_amounts)
      cp_tokens = counterparty_tokens(candidate[:counterparty])
      return true if (cp_tokens & tracked_tokens.to_a).any?

      abs = candidate[:amount_cents]
      tracked_amounts.any? { |ta| ta.present? && (abs - ta).abs.to_f / ta <= 0.03 }
    end

    def to_suggestion(c)
      # Best guess at the lender name: capitalize the normalized counterparty
      lender = c[:counterparty].split.map(&:capitalize).join(" ")

      Suggestion.new(
        lender_guess:            lender,
        source_counterparty:     c[:counterparty].upcase,
        instalment_cents:        c[:amount_cents],
        currency:                c[:currency],
        first_seen_on:           c[:first_seen_on],
        last_seen_on:            c[:last_seen_on],
        count:                   c[:count],
        day_of_month:            c[:day_of_month],
        previous_instalment_cents: c[:previous_amount_cents],
        sample_transaction_ids:  c[:sample_ids],
        key:                     c[:key]
      )
    end
  end
end
