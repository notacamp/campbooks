# frozen_string_literal: true

module Loans
  # Scout spots a loan: a monthly debit with no invoice that goes to a bank (or
  # reads like a loan), seen at least twice. Returns up to two Suggestion structs.
  #
  # Cheap enough to run on every Money page load; memoized per instance.
  class Spotter
    Suggestion = Struct.new(
      :lender_guess, :source_counterparty, :instalment_cents, :currency,
      :first_seen_on, :last_seen_on, :count, :day_of_month,
      :previous_instalment_cents, # set when the amount stepped once (a rate reset)
      :sample_transaction_ids, :key,
      keyword_init: true
    )

    LOAN_KEYWORDS = Loans::Matcher::LOAN_KEYWORDS
    BANKISH = /\b(banco|bank|banque|bcp|millennium|caixa|cgd|santander|novo\s*banco|bpi|bankinter|abanca|montepio|eurobic|bnp|ing|revolut|n26|wise|credit|cr[eé]dito|bbva|sabadell|unicaja|soci[eé]t[eé]\s*g[eé]n[eé]rale|cr[eé]dit\s*agricole|lcl)\b/i

    MAX_SUGGESTIONS         = 2
    CADENCE_MIN_DAYS        = 25
    CADENCE_MAX_DAYS        = 35
    MIN_COUNT               = 2
    MIN_INSTALMENT_CENTS    = 5_000 # a bank fee is monthly and bank-ish too; a loan is not EUR 3.50
    MERGE_AMOUNT_TOLERANCE  = 0.05  # 5% for rate-reset merging
    COVERAGE_RECONCILIATIONS = 24
    # Lines set aside for a specific reason are already explained; only these
    # exclusion reasons may still hide a loan.
    OPEN_EXCLUSION_REASONS  = [ nil, "", "loan", "other" ].freeze

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

      groups = txns.group_by { |txn| [ normalized_counterparty(txn), rounded_cents(txn) ] }
      candidates = groups.filter_map { |(cp, cents), group_txns| build_candidate(cp, cents, group_txns) }
      candidates = merge_on_rate_reset(candidates)

      dismissed = dismissed_suggestion_keys
      tracked_tokens, tracked_amounts = tracked_loan_data
      candidates.reject! { |c| dismissed.include?(c[:key]) || already_tracked?(c, tracked_tokens, tracked_amounts) }

      candidates.sort_by { |c| [ -c[:count], -c[:amount_cents] ] }
                .first(MAX_SUGGESTIONS)
                .map { |c| to_suggestion(c) }
    end

    def candidate_transactions
      recent = @workspace.reconciliations.where(status: :ready).order(created_at: :desc).limit(COVERAGE_RECONCILIATIONS)
      return [] if recent.empty?

      linked = LoanInstalment.where.not(bank_transaction_id: nil).pluck(:bank_transaction_id).to_set
      BankTransaction.where(reconciliation_id: recent.map(&:id))
                     .where(status: %i[unmatched excluded])
                     .where("amount_cents <= ?", -MIN_INSTALMENT_CENTS)
                     .to_a
                     .reject { |t| linked.include?(t.id) }
                     .select { |t| t.unmatched? || OPEN_EXCLUSION_REASONS.include?(t.exclusion_reason) }
    end

    def rounded_cents(txn)
      (txn.amount_cents.abs / 100.0).round * 100
    end

    # A monthly beat, allowing for months whose statement isn't in yet: the gap is
    # a whole number of months (30.44 days each), give or take five days.
    def monthly_gap?(days)
      months = (days / 30.44).round
      months >= 1 && (days - (months * 30.44)).abs <= 5
    end

    def build_candidate(cp, rounded, txns)
      return nil if txns.size < MIN_COUNT

      sorted = txns.sort_by(&:booked_on)
      gaps   = sorted.each_cons(2).map { |a, b| (b.booked_on - a.booked_on).to_i }
      return nil unless gaps.all? { |g| monthly_gap?(g) }

      qualifies = txns.any? do |txn|
        text = "#{txn.description} #{txn.counterparty}"
        LOAN_KEYWORDS.match?(text) || BANKISH.match?(text)
      end
      return nil unless qualifies

      day = sorted.map { |t| t.booked_on.day }.tally.max_by { |_, n| n }&.first
      raw = sorted.map(&:counterparty).compact.reject(&:blank?).tally.max_by { |_, n| n }&.first

      {
        counterparty:  cp,
        raw_counterparty: raw,
        amount_cents:  rounded,
        currency:      txns.first.currency,
        first_seen_on: sorted.first.booked_on,
        last_seen_on:  sorted.last.booked_on,
        count:         txns.size,
        day_of_month:  day,
        sample_ids:    sorted.last(3).map(&:id),
        previous_amount_cents: nil,
        key:           "#{cp}|#{rounded}"
      }
    end

    # Two groups of the same counterparty whose amounts sit within 5% are one loan
    # whose instalment stepped (a rate reset): keep the newer amount as current.
    def merge_on_rate_reset(candidates)
      merged = []
      used   = Set.new

      candidates.each_with_index do |a, i|
        next if used.include?(i)

        partner_index = candidates.each_index.find do |j|
          j != i && !used.include?(j) &&
            candidates[j][:counterparty] == a[:counterparty] &&
            candidates[j][:currency] == a[:currency] &&
            (a[:amount_cents] - candidates[j][:amount_cents]).abs.to_f / [ a[:amount_cents], candidates[j][:amount_cents] ].max <= MERGE_AMOUNT_TOLERANCE
        end

        if partner_index
          b = candidates[partner_index]
          newer, older = [ a, b ].sort_by { |c| c[:last_seen_on] }.reverse
          newer[:previous_amount_cents] = older[:amount_cents] if older[:amount_cents] != newer[:amount_cents]
          newer[:first_seen_on] = [ a[:first_seen_on], b[:first_seen_on] ].min
          newer[:count]        += older[:count]
          newer[:key]           = "#{newer[:counterparty]}|#{newer[:amount_cents]}"
          merged << newer
          used << i << partner_index
        else
          merged << a
          used << i
        end
      end

      merged
    end

    def normalized_counterparty(txn)
      cp = txn.counterparty.presence
      return cp.downcase.gsub(/[^a-z0-9 ]/i, " ").squeeze(" ").strip if cp

      txn.description.downcase.split(/\W+/).reject { |t| t.length < 3 }.first(2).join(" ")
    end

    def dismissed_suggestion_keys
      Array(@workspace.settings.fetch("dismissed_loan_suggestions", []))
    end

    def tracked_loan_data
      active = @workspace.loans.active_loans.to_a
      [ active.flat_map { |l| counterparty_tokens(l.effective_counterparty) }.to_set,
        active.map(&:instalment_cents).to_set ]
    end

    def counterparty_tokens(name)
      name.to_s.downcase.split(/\W+/).reject { |t| t.length < 3 }
    end

    def already_tracked?(candidate, tracked_tokens, tracked_amounts)
      return true if (counterparty_tokens(candidate[:counterparty]) & tracked_tokens.to_a).any?

      abs = candidate[:amount_cents]
      tracked_amounts.any? { |ta| ta.to_i.positive? && (abs - ta).abs.to_f / ta <= 0.03 }
    end

    def to_suggestion(c)
      Suggestion.new(
        lender_guess:              lender_guess(c),
        source_counterparty:       (c[:raw_counterparty].presence || c[:counterparty]).to_s.upcase,
        instalment_cents:          c[:amount_cents],
        currency:                  c[:currency],
        first_seen_on:             c[:first_seen_on],
        last_seen_on:              c[:last_seen_on],
        count:                     c[:count],
        day_of_month:              c[:day_of_month],
        previous_instalment_cents: c[:previous_amount_cents],
        sample_transaction_ids:    c[:sample_ids],
        key:                       c[:key]
      )
    end

    # "MILLENNIUM BCP" reads as "Millennium BCP": words of three letters or fewer
    # stay upper-case (acronyms), the rest are capitalised; a name that already
    # carries lower-case letters is kept as the bank wrote it.
    def lender_guess(c)
      raw = c[:raw_counterparty].presence || c[:counterparty].to_s
      return raw if raw.match?(/[a-z]/)

      raw.split(/\s+/).map { |w| w.length <= 3 ? w.upcase : w.capitalize }.join(" ")
    end
  end
end
