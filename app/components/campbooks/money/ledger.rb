# frozen_string_literal: true

module Campbooks
  module Money
    # The Money ledger — the obligations as a table (desktop) or stacked cards
    # (mobile). Each row is one obligation: counterpart, what Scout read, the signed
    # amount, when it's due, a status chip (icon + label, never colour alone), where
    # it came from, and the affordances the ledger decided on. Settled rows show the
    # bank line and carry no actions.
    class Ledger < Campbooks::Base
      include Campbooks::Money::Glyphs

      MINUS = "−"

      COLUMNS = %i[counterpart what amount due status source].freeze
      # Column → the ledger sort it drives (Money::Ledger::SORTS).
      SORTABLE = { counterpart: :counterpart, amount: :amount, due: :date }.freeze
      # The phone's sort select, in display order.
      ORDERS = %w[priority_desc date_desc date_asc amount_desc amount_asc counterpart_asc counterpart_desc].freeze

      # Strip links above the table.
      STRIP_SORTS = [
        [ :priority, :desc ],
        [ :date,     :desc ],
        [ :amount,   :desc ],
        [ :counterpart, :asc ]
      ].freeze

      def initialize(ledger:, **attrs)
        @ledger = ledger
        @attrs = attrs
      end

      def view_template
        div(id: "money_ledger", class: class_names("mt-2", @attrs.delete(:class)), **@attrs) do
          order_strip
          desktop_table
          mobile_cards
        end
      end

      private

      # ── Order strip ──────────────────────────────────────────────────────────
      def order_strip
        div(class: "mb-3 flex flex-wrap items-center gap-1 text-[12.5px] text-muted-foreground") do
          span(class: "font-medium") { t(".strip.label") }
          span { " · " }
          STRIP_SORTS.each do |sort_key, default_dir|
            active = @ledger.sort == sort_key
            label  = t(".sort.#{sort_key}_#{default_dir}")
            if active
              dir_arrow = @ledger.dir == :asc ? " ↑" : " ↓"
              span(class: "font-semibold text-foreground") { "#{label}#{dir_arrow}" }
            else
              next_dir = ::Money::Ledger.default_dir(sort_key)
              a(href: helpers.money_path(sort: sort_key, dir: next_dir, range: helpers.params[:range].presence),
                class: "hover:text-foreground no-underline") { label }
            end
            span { " · " } unless [ sort_key, default_dir ] == STRIP_SORTS.last
          end
        end
      end

      # ── Desktop ────────────────────────────────────────────────────────────
      def desktop_table
        div(class: "hidden overflow-x-auto sm:block") do
          table(class: "w-full border-collapse text-[13.5px]") do
            thead do
              tr do
                COLUMNS.each { |col| header_cell(col) }
                th(class: header_class) { span(class: "sr-only") { t(".col.actions") } }
              end
            end
            tbody do
              @ledger.sections.each { |(key, list)| desktop_section(key, list) }
            end
          end
        end
      end

      def desktop_section(key, list)
        tr do
          td(colspan: COLUMNS.size + 1, class: "px-3 pb-1 pt-4 text-[11px] font-semibold uppercase tracking-[0.08em] text-muted-foreground") do
            plain "#{t(".section.#{key}")} · #{list.size}"
          end
        end
        list.each { |o| desktop_row(o) }
      end

      def desktop_row(obligation)
        tr(id: obligation.dom_id, class: "border-t border-border/60 align-middle") do
          td(class: "px-3 py-3 font-semibold text-foreground") { obligation.counterpart }
          td(class: "px-3 py-3 text-[12.5px] text-muted-foreground") { what_cell(obligation) }
          td(class: "whitespace-nowrap px-3 py-3 text-right tabular-nums", style: amount_style(obligation)) { amount_text(obligation) }
          td(class: "whitespace-nowrap px-3 py-3 text-[12.5px] text-muted-foreground") { due_text(obligation) }
          td(class: "px-3 py-3") { status_chip(obligation) }
          td(class: "px-3 py-3") { source_cell(obligation) }
          td(class: "px-3 py-3 text-right") { actions_cell(obligation) }
        end
      end

      # A sortable column header is a link that sets the sort (clicking the active
      # column flips its direction); the active one carries aria-sort and an arrow.
      def header_cell(col)
        sort_key = SORTABLE[col]
        right = ("text-right" if col == :amount)
        return th(class: class_names(header_class, right)) { t(".col.#{col}") } unless sort_key

        active = @ledger.sort == sort_key
        dir = active ? @ledger.dir : nil
        next_dir = active ? (dir == :asc ? :desc : :asc) : ::Money::Ledger.default_dir(sort_key)
        th(class: class_names(header_class, right), aria_sort: (dir == :asc ? "ascending" : "descending" if active)) do
          a(href: helpers.money_path(sort: sort_key, dir: next_dir, range: helpers.params[:range].presence),
            class: class_names("inline-flex items-center gap-1 no-underline hover:text-foreground", ("text-foreground" if active)),
            title: t(".sort.by", column: t(".col.#{col}"))) do
            plain t(".col.#{col}")
            if active
              span(aria_hidden: "true") { dir == :asc ? "↑" : "↓" }
            end
          end
        end
      end

      # Phones have no column headers: a small sort select above the cards.
      def mobile_sort_control
        form(action: helpers.money_path, method: :get, class: "flex items-center justify-end gap-2",
             data: { controller: "auto-submit" }) do
          input(type: "hidden", name: "range", value: helpers.params[:range]) if helpers.params[:range].present?
          label(for: "money_order", class: "text-[11.5px] text-muted-foreground") { t(".sort.label") }
          select(id: "money_order", name: "order",
                 class: "rounded-md border border-border bg-card px-2 py-1 text-[12.5px] text-foreground",
                 data: { action: "change->auto-submit#submit" }) do
            ORDERS.each do |order|
              option(value: order, selected: (order == "#{@ledger.sort}_#{@ledger.dir}")) { t(".sort.#{order}") }
            end
          end
        end
      end

      # ── Mobile ─────────────────────────────────────────────────────────────
      def mobile_cards
        div(class: "space-y-4 sm:hidden") do
          mobile_sort_control
          @ledger.sections.each do |(key, list)|
            div(class: "text-[11px] font-semibold uppercase tracking-[0.08em] text-muted-foreground") { "#{t(".section.#{key}")} · #{list.size}" }
            div(class: "space-y-2.5") { list.each { |o| mobile_card(o) } }
          end
        end
      end

      def mobile_card(obligation)
        div(id: "m-#{obligation.dom_id}", class: "rounded-2xl border border-border bg-card p-3.5") do
          div(class: "flex items-start justify-between gap-3") do
            div(class: "min-w-0") do
              div(class: "font-semibold text-foreground") { obligation.counterpart }
              div(class: "mt-0.5 text-[12.5px] text-muted-foreground") { obligation.what }
              why_line(obligation) unless obligation.settled?
            end
            div(class: "whitespace-nowrap text-right tabular-nums", style: amount_style(obligation)) { amount_text(obligation) }
          end
          div(class: "mt-2.5 flex flex-wrap items-center gap-2") do
            status_chip(obligation)
            span(class: "text-[12.5px] text-muted-foreground") { due_text(obligation) }
            span(class: "ml-auto") { source_cell(obligation) }
          end
          actions = actions_cell(obligation, justify: "start")
          div(class: "mt-2.5") { actions } if obligation.actions.any?
        end
      end

      # ── Cells ──────────────────────────────────────────────────────────────
      def what_cell(obligation)
        div do
          plain obligation.what
          if obligation.due_estimated?
            whitespace
            span(class: "text-muted-foreground/70") { "· #{t('money.status.estimated')}" }
          end
          if obligation.recurring? && obligation.next_renewal_on
            div(class: "text-[11.5px] text-muted-foreground/70") { t(".next_renews", date: l(obligation.next_renewal_on, format: :date)) }
          end
          why_line(obligation) unless obligation.settled?
        end
      end

      # A 11.5px reason line under the what cell for open obligations.
      def why_line(obligation)
        why = Array(obligation.why)
        return if why.empty?

        weight = obligation.counterpart_weight.to_f
        dot_class = weight >= 0.6 ? "bg-ember-gradient" : "bg-muted-foreground/40"
        div(class: "mt-0.5 flex items-center gap-1 text-[11.5px] text-muted-foreground/80") do
          span(class: "inline-block h-1.5 w-1.5 shrink-0 rounded-full #{dot_class}")
          plain why.join(" · ")
        end
      end

      def amount_text(obligation)
        sign = obligation.receivable? ? "+ " : "#{MINUS} "
        plain "#{sign}#{obligation.amount&.format}"
      end

      def amount_style(obligation)
        "font-weight:500;#{'color:var(--muted-foreground)' if obligation.settled?}"
      end

      def due_text(obligation)
        return "" unless obligation.due_on

        l(obligation.due_on, format: :date)
      end

      # ── Status chip (icon + label, always) ───────────────────────────────────
      def status_chip(obligation)
        case obligation.status
        when :late    then chip(:warning, :warning, t("money.status.late", count: obligation.days_late(Date.current)))
        when :settled then chip(:success, :check, t("money.status.paid", date: l(obligation.settled_on, format: :date)))
        when :decide  then chip(:ember, :spark, t("money.status.decide"))
        else               chip(:muted, :clock, due_chip_label(obligation))
        end
      end

      def due_chip_label(obligation)
        days = (obligation.due_on.to_date - Date.current).to_i
        return t("money.status.due_today") if days <= 0

        t("money.status.due", count: days)
      end

      TONE_CLASSES = {
        warning: "bg-warning/10 text-warning",
        success: "bg-success/10 text-success",
        muted:   "bg-secondary text-muted-foreground",
        ember:   "scout-glass text-foreground"
      }.freeze

      def chip(tone, icon, label)
        span(class: class_names("inline-flex h-[22px] items-center gap-1.5 whitespace-nowrap rounded-md px-2 text-[11.5px] font-medium", TONE_CLASSES[tone])) do
          money_icon(icon, css_class: class_names("h-3.5 w-3.5", ("text-ember" if tone == :ember)), fill: icon == :spark)
          plain label
        end
      end

      # ── Source ───────────────────────────────────────────────────────────────
      def source_cell(obligation)
        if obligation.settled? && obligation.settled_via.present?
          return span(class: "whitespace-nowrap text-[12.5px] text-muted-foreground") { obligation.settled_via }
        end

        div(class: "flex items-center gap-2 text-muted-foreground") do
          if obligation.source_email_message
            a(href: helpers.email_message_path(obligation.source_email_message), class: "hover:text-foreground", aria_label: t(".source_email")) { money_icon(:mail, css_class: "h-[18px] w-[18px]") }
          end
          if obligation.document
            a(href: helpers.document_path(obligation.document), class: "hover:text-foreground", aria_label: t(".source_document")) { money_icon(:file, css_class: "h-[18px] w-[18px]") }
          end
        end
      end

      # ── Actions ──────────────────────────────────────────────────────────────
      def actions_cell(obligation, justify: "end")
        div(class: "flex items-center gap-1.5 justify-#{justify}") do
          obligation.actions.each { |action| action_button(obligation, action) }
        end
      end

      def action_button(obligation, action)
        case action
        when :mark_paid
          settle_button(obligation)
        when :send_reminder
          post_form(helpers.money_obligation_chase_path(obligation.id)) do
            render(Campbooks::Button.new(variant: :primary, size: :xs, type: "submit")) do
              money_icon(:spark, css_class: "h-3 w-3", fill: true)
              plain t(".send_reminder")
            end
          end
        when :pay
          render(Campbooks::Button.new(variant: :primary, size: :xs, href: obligation.pay_url, target: "_blank", rel: "noopener")) { t(".pay") }
        when :remind_on
          post_form(helpers.money_obligation_remind_path(obligation.id)) do
            render(Campbooks::Button.new(variant: :outline, size: :xs, type: "submit")) { t(".remind_on", date: l((obligation.due_on || Date.current) + 1, format: :day_month)) }
          end
        when :keep
          decide_button(obligation, "keep", :outline, t(".keep"))
        when :cancel
          decide_button(obligation, "cancel", :outline, t(".cancel"))
        end
      end

      def settle_button(obligation)
        post_form(helpers.money_obligation_settle_path(obligation.id)) do
          render(Campbooks::Button.new(variant: :outline, size: :xs, type: "submit")) { t(".mark_paid") }
        end
      end

      def decide_button(obligation, choice, variant, label)
        post_form(helpers.money_obligation_decide_path(obligation.id), hidden: { choice: choice }) do
          render(Campbooks::Button.new(variant: variant, size: :xs, type: "submit")) { label }
        end
      end

      def post_form(action, hidden: {})
        form(action: action, method: :post, class: "inline-flex") do
          input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
          hidden.each { |name, value| input(type: "hidden", name: name.to_s, value: value.to_s) }
          yield
        end
      end

      def header_class
        "border-b border-border px-3 pb-2 text-left text-[11px] font-semibold uppercase tracking-[0.08em] text-muted-foreground"
      end
    end
  end
end
