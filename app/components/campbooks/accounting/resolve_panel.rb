# frozen_string_literal: true

module Campbooks
  module Accounting
    # Structural wrapper for the transaction resolve panel.
    # The controller action renders this component inside a Turbo Frame;
    # the component itself renders sub-partials for the form sections.
    #
    # Sections:
    #   ① Scout's suggestions (suggested matches + Confirm/Reject)
    #   ② Search your documents (list-search + manual_match)
    #   ③ No document needed (exclusion reason + exclude action)
    #   ④ Request invoice — PR 3 seam (not rendered)
    #
    # @param transaction         [BankTransaction]
    # @param reconciliation      [Reconciliation]
    # @param suggested_matches   [Array<TransactionMatch>]
    # @param candidate_documents [Array<Document>]
    # @param company_nif         [String, nil]
    class ResolvePanel < Campbooks::Base
      EXCLUSION_REASONS = %w[bank_fee salary transfer tax other].freeze

      def initialize(transaction:, reconciliation:, suggested_matches: [],
                     candidate_documents: [], company_nif: nil)
        @transaction         = transaction
        @reconciliation      = reconciliation
        @suggested_matches   = Array(suggested_matches)
        @candidate_documents = Array(candidate_documents)
        @company_nif         = company_nif
      end

      def view_template
        div(class: "p-4 space-y-6") do
          transaction_recap
          suggestions_section if @suggested_matches.any?
          search_section
          exclude_section
        end
      end

      private

      # ── Transaction recap ─────────────────────────────────────────────────────

      def transaction_recap
        div(class: "bg-muted/40 rounded-lg p-3") do
          div(class: "flex items-start justify-between gap-2") do
            div(class: "min-w-0 flex-1") do
              p(class: "text-sm font-medium text-foreground") { @transaction.description }
              p(class: "text-xs text-muted-foreground mt-0.5") do
                parts = [ l(@transaction.booked_on, format: :date) ]
                parts << @transaction.counterparty if @transaction.counterparty.present?
                plain parts.join(" · ")
              end
            end
            div(class: "shrink-0 text-right") do
              span(class: "text-sm font-semibold tabular-nums #{amount_color}") do
                amount_text
              end
            end
          end
        end
      end

      def amount_color
        @transaction.debit? ? "text-red-600 dark:text-red-400" : "text-green-700 dark:text-green-400"
      end

      def amount_text
        sign = @transaction.debit? ? "−" : "+"
        num  = helpers.number_to_currency(@transaction.amount_cents.abs / 100.0, unit: "", precision: 2)
        "#{sign}#{num} #{@transaction.currency}"
      end

      # ── Section 1: Scout suggestions ─────────────────────────────────────────

      def suggestions_section
        div(class: "space-y-3") do
          section_header_with_icon(t(".suggestions_title"))

          @suggested_matches.each { |match| suggestion_card(match) }

          p(class: "text-xs text-muted-foreground") do
            t(".checked_documents", count: @transaction.transaction_matches.size)
          end
        end
      end

      def suggestion_card(match)
        doc        = match.document
        nif_status = @company_nif ? doc.nif_status(@company_nif) : nil

        div(class: "border border-border rounded-lg p-3 space-y-2 bg-card") do
          render(Campbooks::Accounting::MatchChip.new(match: match, nif_status: nif_status))
          render(Campbooks::Accounting::ConfidenceBadge.new(match: match, expandable: true))

          div(class: "flex gap-2 pt-1") do
            confirm_url = helpers.confirm_reconciliation_bank_transaction_path(@reconciliation, @transaction)
            reject_url  = helpers.reject_reconciliation_bank_transaction_path(@reconciliation, @transaction)

            raw helpers.button_to(
              t(".use_this_match"),
              confirm_url,
              method: :post,
              params: { match_id: match.id },
              class: button_classes(:primary)
            )
            raw helpers.button_to(
              t(".not_this_one"),
              reject_url,
              method: :post,
              params: { match_id: match.id },
              class: button_classes(:ghost)
            )
          end
        end
      end

      # ── Section 2: Document search ────────────────────────────────────────────

      def search_section
        div(class: "space-y-2") do
          h3(class: "text-sm font-semibold text-foreground") { t(".search_title") }

          frame_id = "resolve_doc_search_#{@transaction.id}"
          search_url = helpers.resolve_panel_reconciliation_bank_transaction_path(
            @reconciliation, @transaction, format: :turbo_stream
          )

          # Search form (outside the frame so it persists across frame refreshes)
          raw helpers.form_with(url: search_url, method: :get,
                                data: { controller: "list-search",
                                        "turbo-frame": frame_id },
                                class: "mb-2") do |f|
            helpers.tag.div(class: "relative") do
              f.text_field(:q,
                placeholder: t(".search_placeholder"),
                class:       "block w-full rounded-lg border-border bg-card text-sm px-3 py-2",
                data:        { list_search_target: "input",
                               action: "input->list-search#submit" })
            end
          end

          # Lazy-loaded document list inside a Turbo Frame
          raw helpers.turbo_frame_tag(frame_id,
                                      src: search_url,
                                      class: "block space-y-1 max-h-48 overflow-y-auto") do
            doc_list_items.html_safe
          end
        end
      end

      def doc_list_items
        @candidate_documents.map { |doc| doc_row(doc) }.join
      end

      def doc_row(doc)
        manual_url = helpers.manual_match_reconciliation_bank_transaction_path(@reconciliation, @transaction)
        helpers.tag.div(class: "flex items-center justify-between gap-2 px-2 py-1.5 rounded hover:bg-muted/40") do
          helpers.tag.div(class: "min-w-0 flex-1") do
            helpers.tag.p(class: "text-xs font-medium truncate") { doc.display_title } +
            helpers.tag.p(class: "text-[10px] text-muted-foreground") { doc_meta(doc) }
          end +
          helpers.button_to(
            t(".attach"),
            manual_url,
            method: :post,
            params: { document_id: doc.id },
            class: "shrink-0 text-xs font-medium text-accent-600 hover:text-accent-700"
          )
        end
      end

      def doc_meta(doc)
        parts = [ doc.classification&.name || doc.document_type.humanize ]
        if doc.amount_cents.present?
          parts << helpers.number_to_currency(doc.amount_cents / 100.0, unit: "", precision: 2)
        end
        parts << helpers.l(doc.document_date, format: :date) if doc.document_date.present?
        parts.join(" · ")
      end

      # ── Section 3: No document needed ────────────────────────────────────────

      def exclude_section
        exclude_url = helpers.exclude_reconciliation_bank_transaction_path(@reconciliation, @transaction)
        div(class: "space-y-2") do
          h3(class: "text-sm font-semibold text-foreground") { t(".exclude_title") }
          p(class: "text-xs text-muted-foreground") { t(".exclude_hint") }

          raw helpers.form_with(url: exclude_url, method: :post, class: "flex gap-2 items-end") do |f|
            options = EXCLUSION_REASONS.map do |r|
              [ t("reconciliations.bank_transactions.exclusion_reasons.#{r}"), r ]
            end
            helpers.tag.div(class: "flex-1") do
              f.select(:reason, options,
                       {},
                       class: "block w-full rounded-lg border-border bg-card text-sm px-3 py-2")
            end +
            f.submit(t(".mark_excluded"),
                     class: "shrink-0 inline-flex items-center px-3 py-2 text-xs font-medium rounded-lg border border-border bg-card hover:bg-muted/40")
          end
        end
      end

      # ── Helpers ──────────────────────────────────────────────────────────────

      def section_header_with_icon(title)
        div(class: "flex items-center gap-2") do
          # Scout avatar icon
          div(class: "w-6 h-6 rounded-full bg-accent-100 dark:bg-accent-900 flex items-center justify-center shrink-0") do
            svg(class: "w-3.5 h-3.5 text-accent-600", fill: "none", viewBox: "0 0 24 24", stroke: "currentColor") do |s|
              s.path(stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "2",
                     d: "M9.663 17h4.673M12 3v1m6.364 1.636l-.707.707M21 12h-1M4 12H3m3.343-5.657l-.707-.707m2.828 9.9a5 5 0 117.072 0l-.548.547A3.374 3.374 0 0014 18.469V19a2 2 0 11-4 0v-.531c0-.895-.356-1.754-.988-2.386l-.548-.547z")
            end
          end
          h3(class: "text-sm font-semibold text-foreground") { title }
        end
      end

      # Fix 13c: derive button CSS from Campbooks::Button constants so the resolve
      # panel matches the rest of the UI and doesn't drift when token values change.
      def button_classes(variant, size: :xs)
        class_names(
          Campbooks::Button::BASE_CLASSES,
          Campbooks::Button::VARIANT_CLASSES[variant],
          Campbooks::Button::SIZE_CLASSES[size]
        )
      end
    end
  end
end
