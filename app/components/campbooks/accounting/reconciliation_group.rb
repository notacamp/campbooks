# frozen_string_literal: true

module Campbooks
  module Accounting
    # Renders one reconciliation group: bank line(s) ↔ state node ↔ invoice(s).
    #
    # Layout follows the approved prototype:
    #   - Simple 1:1  → flat row, no inset box
    #   - Multi-item  → subtle inset container with a balance footer
    #   - Partial     → inset container with amber "outstanding" footer
    #   - Excluded    → muted node + "Set aside · <reason>" label
    #   - Unmatched   → ember "!" node + "No invoice · Resolve" chip
    #   - Credit      → green signed amount
    #   - Review      → amber "~" node + confidence% + NIF flag
    #
    # @param group          [Reconciliations::Groups::Group]
    # @param company_nif   [String, nil]
    # @param reconciliation [Reconciliation, nil]  — needed to open the hunt panel
    class ReconciliationGroup < Campbooks::Base
      # How many txns/docs before we show a ratio label (e.g. "2 → 1").
      MULTI_THRESHOLD = 1

      def initialize(group:, company_nif: nil, reconciliation: nil)
        @group          = group
        @company_nif    = company_nif.presence
        @reconciliation = reconciliation
      end

      def view_template
        multi = multi_item? || group_state == :partial

        if multi
          # Inset container for complex groups
          div(class: "py-3", **hunt_data_attrs) do
            div(class: "rounded-2xl border border-border bg-muted/40 px-4 py-4 space-y-0") do
              row_grid { group_body }
              balance_footer if show_footer?
            end
          end
        else
          # Flat row for simple 1:1 / excluded / unmatched / requested
          div(class: "py-3.5", **hunt_data_attrs) do
            row_grid { group_body }
          end
        end
      end

      private

      # ── Layout ─────────────────────────────────────────────────────────────

      def row_grid
        div(class: "grid grid-cols-1 gap-y-3 sm:grid-cols-[minmax(0,1fr)_80px_minmax(0,1fr)] sm:gap-x-4 sm:gap-y-0 sm:items-center") do
          yield
        end
      end

      def group_body
        bank_side
        middle_node
        invoice_side
      end

      # ── Bank (left) side ───────────────────────────────────────────────────

      def bank_side
        div(class: "flex flex-col gap-2 justify-center") do
          @group.bank_transactions.each { |txn| bank_line(txn) }
        end
      end

      def bank_line(txn)
        div(class: "flex items-baseline justify-between gap-2") do
          div(class: "min-w-0") do
            div(class: "text-[13.5px] font-medium text-foreground truncate") { txn.description }
            div(class: "mt-0.5 text-[12px] text-muted-foreground") do
              plain l(txn.booked_on, format: :date)
              if txn.counterparty.present?
                plain " · "
                plain txn.counterparty
              end
            end
          end
          span(class: class_names("text-[13px] font-semibold whitespace-nowrap tabular-nums", amount_color(txn))) do
            plain signed_amount(txn)
          end
        end
      end

      # ── Middle node ────────────────────────────────────────────────────────

      def middle_node
        div(class: "flex flex-row items-center justify-start gap-2 sm:flex-col sm:justify-center sm:gap-1") do
          node_glyph
          ratio_label if show_ratio_label?
        end
      end

      def node_glyph
        glyph, title_text, klass = node_attrs
        div(class: class_names(
              "w-6 h-6 rounded-full flex items-center justify-center text-[13px] font-bold",
              klass
            ), title: title_text) do
          plain glyph
        end
      end

      # Returns [glyph_string_or_svg, title, tailwind_classes]
      def node_attrs
        case group_state
        when :matched
          [ check_svg, t(".state.matched"), "bg-success/15 text-success" ]
        when :review
          [ "~", t(".state.review"), "bg-warning/15 text-warning" ]
        when :partial
          [ "½", t(".state.partial"), "bg-warning/15 text-warning" ]
        when :excluded
          [ "—", t(".state.excluded"), "bg-muted text-muted-foreground" ]
        when :requested
          [ "→", t(".state.requested"), "bg-muted text-muted-foreground" ]
        when :credit
          [ check_svg, t(".state.credit"), "bg-success/15 text-success" ]
        else # :unmatched
          [ "!", t(".state.unmatched"), "bg-ember/15 text-ember" ]
        end
      end

      def check_svg
        raw(safe('<svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><path d="M20 6 9 17l-5-5"/></svg>'))
      end

      def show_ratio_label?
        case group_state
        when :review
          # show confidence %
          true
        when :matched, :credit, :partial
          multi_item? # only show ratio on complex groups
        else
          false
        end
      end

      def ratio_label
        label = case group_state
        when :review  then "#{confidence_pct}%"
        when :partial then t(".state.partial_label")
        else               "#{@group.bank_transactions.size} → #{@group.documents.size}"
        end
        span(class: "text-[10.5px] text-muted-foreground font-semibold whitespace-nowrap") { plain label }
      end

      # ── Invoice (right) side ───────────────────────────────────────────────

      def invoice_side
        div(class: "flex flex-col gap-2 justify-center") do
          case group_state
          when :excluded
            excluded_label
          when :requested
            requested_label
          when :unmatched
            unmatched_chip
          else
            @group.documents.each { |doc| invoice_line(doc) }
          end
        end
      end

      def invoice_line(doc)
        div(class: "flex items-baseline justify-between gap-2") do
          div(class: "min-w-0") do
            div(class: "flex items-center gap-1.5 flex-wrap") do
              span(class: "text-[13px] font-medium text-foreground") { plain party_name(doc) }
              nif_badge(doc) if nif_flagged?(doc)
            end
            div(class: "mt-0.5 text-[12px] text-muted-foreground truncate") do
              parts = []
              parts << doc.invoice_number if doc.invoice_number.present?
              parts << t(".revenue_label") if doc.revenue_invoice?
              parts << t(".review_label")  if group_state == :review
              plain parts.join(" · ")
            end
          end
          span(class: "text-[12.5px] font-medium text-muted-foreground whitespace-nowrap tabular-nums") do
            plain format_amount_cents(doc.amount_cents, doc.currency)
          end
        end
      end

      def excluded_label
        reason = excluded_reason_label
        span(class: "text-[13px] text-muted-foreground") do
          plain t(".excluded_label")
          plain " · #{reason}" if reason.present?
        end
      end

      def unmatched_chip
        button(
          class: "inline-flex items-center gap-1.5 rounded-lg px-2.5 py-1 text-[12.5px] font-semibold bg-ember/10 text-ember cursor-pointer hover:bg-ember/15 transition-colors",
          data:  { action: "click->transaction-resolve#toggle" }
        ) do
          plain t(".unmatched_chip")
        end
      end

      def requested_label
        span(class: "text-[13px] text-muted-foreground") do
          plain t(".requested_label")
        end
      end

      # Emits Stimulus controller data on the group's root div so the
      # "Resolve" chip can open the hunt panel via turbo_frame (same
      # transaction-resolve controller used by the classic workbench table).
      def hunt_data_attrs
        return {} unless group_state == :unmatched

        txn = @group.bank_transactions.first
        return {} unless txn && @reconciliation

        {
          id:   helpers.dom_id(txn),
          data: {
            controller:                        "transaction-resolve",
            transaction_resolve_url_value:      helpers.resolve_panel_reconciliation_bank_transaction_path(
                                                  @reconciliation, txn),
            transaction_resolve_frame_id_value: helpers.dom_id(txn, :resolve_frame)
          }
        }
      end

      # ── Balance footer (multi-item groups) ─────────────────────────────────

      def balance_footer
        div(class: "col-span-full mt-3 pt-3 border-t border-dashed border-border flex flex-wrap items-center gap-x-3 gap-y-1 text-[12.5px] text-muted-foreground") do
          footer_narrative
          whitespace
          footer_balance
        end
      end

      def show_footer?
        group_state == :partial || (multi_item? && group_state.in?(%i[matched credit]))
      end

      def footer_narrative
        case group_state
        when :partial
          txn_count = @group.bank_transactions.size
          doc_count  = @group.documents.size
          plain(txn_count > 1 ? t(".footer.installments") : (doc_count > 1 ? t(".footer.split") : t(".footer.partial_deposit")))
        else
          txn_count = @group.bank_transactions.size
          doc_count  = @group.documents.size
          if txn_count > 1
            plain t(".footer.multi_txn", count: txn_count)
          else
            plain t(".footer.multi_doc", count: doc_count)
          end
        end
      end

      def footer_balance
        case group_state
        when :partial
          outstanding_s = format_amount_cents(@group.outstanding_cents, primary_currency)
          allocated_s   = format_amount_cents(@group.allocated_cents,   primary_currency)
          total_s       = format_amount_cents(@group.invoice_total_cents, primary_currency)
          span(class: "font-semibold text-warning") do
            plain "#{allocated_s} #{t(".footer.of")} #{total_s} · #{outstanding_s} #{t(".footer.outstanding")}"
          end
        else
          line_s = format_amount_cents(@group.line_total_cents, primary_currency)
          if @group.bank_transactions.size > 1
            parts = @group.bank_transactions.map { |t| format_amount_cents(t.amount_cents.abs, t.currency) }
            span(class: "font-semibold text-success") do
              plain "#{parts.join(" + ")} = #{line_s} ✓"
            end
          else
            span(class: "font-semibold text-success") do
              plain "#{line_s} · #{t(".footer.balances")} ✓"
            end
          end
        end
      end

      # ── State detection ─────────────────────────────────────────────────────

      def group_state
        @group_state ||= begin
          txns = @group.bank_transactions
          docs = @group.documents

          if docs.empty?
            if txns.all?(&:excluded?)
              :excluded
            elsif txns.all?(&:requested?)
              :requested
            else
              :unmatched
            end
          elsif @group.kind == :partial
            :partial
          elsif txns.any?(&:credit?) && txns.all?(&:credit?)
            :credit
          elsif top_match_suggested?
            :review
          else
            :matched
          end
        end
      end

      def top_match_suggested?
        @group.bank_transactions.any? do |txn|
          txn.transaction_matches.any?(&:suggested?)
        end
      end

      def multi_item?
        @group.bank_transactions.size > 1 || @group.documents.size > 1
      end

      # ── NIF / Confidence ────────────────────────────────────────────────────

      def nif_flagged?(doc)
        return false if @company_nif.blank?

        doc.nif_status(@company_nif)&.in?(%i[missing mismatch])
      end

      def nif_badge(doc)
        status = doc.nif_status(@company_nif)
        title  = status == :mismatch ? t(".nif_mismatch") : t(".nif_missing")
        span(class: "shrink-0 text-[10px] font-bold text-warning border border-warning/40 rounded px-1",
             title: title) { plain "NIF" }
      end

      def confidence_pct
        # Pull the best suggested match confidence across all transactions in the group
        best = @group.bank_transactions.flat_map(&:transaction_matches)
                     .select(&:suggested?)
                     .map(&:confidence)
                     .compact
                     .max
        return 0 unless best

        (best * 100).round
      end

      # ── Helpers ─────────────────────────────────────────────────────────────

      def signed_amount(txn)
        sign = txn.credit? ? "+" : "−"
        "#{sign}#{format_amount_cents(txn.amount_cents.abs, txn.currency)}"
      end

      def amount_color(txn)
        txn.credit? ? "text-success" : "text-foreground"
      end

      def format_amount_cents(cents, currency)
        return "—" if cents.nil?

        symbol = currency_symbol(currency.to_s)
        "#{symbol}#{sprintf("%.2f", cents.abs / 100.0)}"
      end

      def currency_symbol(code)
        { "EUR" => "€", "USD" => "$", "GBP" => "£", "BRL" => "R$" }.fetch(code.upcase, "#{code} ")
      end

      def party_name(doc)
        doc.vendor_name.presence || doc.client_name.presence || t(".unknown_party")
      end

      def primary_currency
        @group.bank_transactions.first&.currency || "EUR"
      end

      def excluded_reason_label
        txn = @group.bank_transactions.first
        return "" unless txn

        # exclusion_reason stored in metadata or description
        metadata = txn.respond_to?(:exclusion_reason) ? txn.exclusion_reason : nil
        metadata.present? ? t("reconciliations.bank_transactions.exclusion_reasons.#{metadata}", default: metadata) : ""
      end
    end
  end
end
