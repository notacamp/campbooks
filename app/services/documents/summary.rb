# frozen_string_literal: true

module Documents
  # Scout's one-line read of what is open on Paper — the note row at the top of the
  # page. It states ONLY what the documents prove (docs/messaging.md: never claim Scout
  # "handled" anything): how many invoices are unpaid and their total, the worst overdue
  # counterpart, and how many documents want a second look.
  #
  #   summary = Documents::Summary.for(workspace, user)
  #   summary.sentence           # "Two invoices are unpaid, €612.00 in total. …"
  #   summary.needs_review_count # drives the "Review it/them" button
  #   summary.any?               # false → "nothing needs you" (or the connect prompt)
  #   summary.ai_configured?     # false + !any? → "Connect an AI provider…"
  class Summary
    def self.for(workspace, user, today: Date.current)
      new(workspace, user, today)
    end

    def initialize(workspace, user, today = Date.current)
      @workspace = workspace
      @user = user
      @today = today
    end

    # Open obligations: money documents with an amount and a due date that aren't
    # settled — i.e. their derived status is :unpaid or :late (needs-review money docs
    # read as :needs_review and are deliberately excluded here).
    def obligations
      @obligations ||= candidate_money_documents.filter_map do |doc|
        status = Documents::Status.for(doc, today: @today).status
        next unless status == :unpaid || status == :late

        { document: doc, status: status, days: overdue_days(doc) }
      end
    end

    def unpaid_count
      obligations.size
    end

    def late
      obligations.select { |o| o[:status] == :late }
    end

    def late_count
      late.size
    end

    # { "EUR" => 61_200, … } — cents per currency across the open obligations.
    def totals_by_currency
      @totals_by_currency ||= obligations.each_with_object(Hash.new(0)) do |o, acc|
        doc = o[:document]
        acc[doc.currency] += doc.amount_cents.to_i
      end
    end

    def worst_late
      late.max_by { |o| o[:days] }
    end

    def needs_review_count
      @needs_review_count ||= workspace_documents.needs_review.count
    end

    def ai_configured?
      return @ai_configured if defined?(@ai_configured)

      @ai_configured = @workspace.present? &&
                       Ai::ProviderSetup.configured?(@workspace, :document_analysis)
    end

    def any?
      unpaid_count.positive? || needs_review_count.positive?
    end

    # The assembled note-row sentence (plain text), from pluralized i18n fragments.
    # nil when there is nothing to say AND AI isn't configured — the note row then
    # renders the "Connect an AI provider…" prompt instead.
    def sentence
      return nil if !any? && !ai_configured?

      fragments = []
      fragments << unpaid_fragment if unpaid_count.positive?
      fragments << overdue_fragment if worst_late
      fragments << needs_review_fragment if needs_review_count.positive?
      fragments << I18n.t("paper.summary.clear") if fragments.empty?
      fragments.join(" ")
    end

    private

    def unpaid_fragment
      I18n.t("paper.summary.unpaid", count: unpaid_count, total: formatted_total)
    end

    def overdue_fragment
      days = I18n.t("paper.status.days", count: worst_late[:days])
      I18n.t("paper.summary.overdue", name: worst_late[:document].entity_display_name, days: days)
    end

    def needs_review_fragment
      I18n.t("paper.summary.needs_review", count: needs_review_count)
    end

    def formatted_total
      totals_by_currency.map { |currency, cents| Money.new(cents, currency).format }.join(" + ")
    end

    def overdue_days(doc)
      due = due_date(doc)
      due && due < @today ? (@today - due).to_i : 0
    end

    def due_date(doc)
      value = doc.due_date
      value.respond_to?(:to_date) ? value.to_date : nil
    rescue StandardError
      nil
    end

    # Money documents that could be open obligations: not settled. Bounded in practice
    # (a workspace's live invoices); status is then derived in Ruby (it isn't indexed).
    def candidate_money_documents
      workspace_documents.money_types.where(settled_at: nil).to_a
    end

    def workspace_documents
      return Document.none unless @workspace

      @workspace.documents.accessible_to(@user)
    end
  end
end
