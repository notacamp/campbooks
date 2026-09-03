# frozen_string_literal: true

module Campbooks
  module Money
    # The 30-day timeline: two directions on one axis. Owed to you above the line,
    # you owe below; the next N days ahead and the overdue to the left of today.
    # Every bar is one obligation at its due date (settled ones at the date the bank
    # cleared them), linked to its ledger row. Pure inline SVG — no JS library — that
    # scrolls horizontally on a phone inside its own container.
    #
    # Because a workspace can carry a deep backlog of very old bills, the axis does
    # NOT stretch to fit them (they would clamp onto a single pixel and overprint).
    # Anything dated before the window folds into one "N older" marker per lane in a
    # left gutter, and anything dated after it into an "N later" marker on the right;
    # both link to the ledger. The axis stays legible at any backlog size, and when
    # there is no overflow it is pixel-identical to a bare 21-day-back / 30-day-ahead
    # window.
    #
    # Colour follows status (ink / destructive / muted), but every late marker also
    # carries a warning glyph, so colour is never the only signal (validated for
    # protanopia in the mock).
    class Timeline < Campbooks::Base
      include Campbooks::Money::Glyphs

      W = 1120
      H = 258
      AXIS_Y = 125
      LANE_H = 100.0
      PLOT_LEFT = 132
      PLOT_RIGHT = 1108
      TICK_Y = 244
      BAR_W = 10
      MIN_BAR = 8
      MAX_BAR = LANE_H
      # Reserved bands for the overflow markers (only when that side overflows).
      GUTTER = 150   # left: obligations older than the window
      TAIL = 118     # right: obligations later than the window
      # Vertical centre of each lane's overflow marker.
      MARKER_Y = { up: 72, down: 178 }.freeze

      def initialize(ledger:, summary:, today: Date.current, **attrs)
        @ledger = ledger
        @summary = summary
        @today = today
        @attrs = attrs
      end

      def view_template
        div(class: class_names("mt-1", @attrs.delete(:class)), **@attrs) do
          header
          div(class: "mt-2 overflow-x-auto") do
            div(class: "min-w-[720px]") { raw safe(svg) }
          end
        end
      end

      private

      def header
        div(class: "flex flex-wrap items-center gap-x-2.5 gap-y-1") do
          span(class: "text-[13px] font-semibold text-foreground") { t(".title", days: horizon_days) }
          span(class: "text-[12px] text-muted-foreground") { t(".hint") }
          div(class: "ml-auto flex items-center gap-3.5 text-[11.5px] text-muted-foreground") do
            legend_key("open", "var(--foreground)")
            legend_key("late", "var(--destructive)")
            legend_key("settled", "var(--muted-foreground)")
          end
        end
      end

      def legend_key(kind, color)
        span(class: "inline-flex items-center gap-1.5") do
          span(class: "inline-block h-2.5 w-2.5 rounded-[3px]", style: "background:#{color}#{';opacity:.5' if kind == 'settled'}")
          plain t(".legend.#{kind}")
        end
      end

      # ── SVG ──────────────────────────────────────────────────────────────────
      def svg
        %(<svg viewBox="0 0 #{W} #{H}" width="100%" height="#{H}" role="img" aria-label="#{esc(t('.title', days: horizon_days))}" class="text-foreground">) +
          shaded_overdue + grid + axis + today_marker + overflow_dividers + lane_labels +
          bars_svg + overflow_markers +
          "</svg>"
      end

      def shaded_overdue
        x = position(@today)
        %(<rect x="#{axis_left}" y="20" width="#{(x - axis_left).round(1)}" height="210" rx="8" fill="var(--secondary)" opacity="0.6"/>)
      end

      def grid
        ticks.map do |date|
          x = position(date).round(1)
          %(<line x1="#{x}" y1="22" x2="#{x}" y2="230" stroke="var(--border)" stroke-width="1" opacity="0.6"/>) +
            %(<text x="#{x}" y="#{TICK_Y}" text-anchor="middle" font-family="JetBrains Mono, monospace" font-size="10.5" fill="var(--muted-foreground)">#{esc(l(date, format: :day_month))}</text>)
        end.join
      end

      def axis
        %(<line x1="#{PLOT_LEFT}" y1="#{AXIS_Y}" x2="#{PLOT_RIGHT}" y2="#{AXIS_Y}" stroke="var(--border)" stroke-width="1"/>)
      end

      def today_marker
        x = position(@today).round(1)
        %(<line x1="#{x}" y1="18" x2="#{x}" y2="232" stroke="var(--foreground)" stroke-width="1" opacity="0.55"/>) +
          %(<text x="#{x + 5}" y="24" font-size="12" font-weight="600" fill="var(--foreground)">#{esc(t('.today'))}</text>)
      end

      def lane_labels
        [
          lane_label(52, t(".owed"), lane_total(@summary.owed_to_you_total)),
          lane_label(158, t(".owe"), lane_total(@summary.you_owe_total))
        ].join
      end

      def lane_label(y, name, total)
        %(<text x="6" y="#{y}" font-size="13" font-weight="600" fill="var(--foreground)">#{esc(name)}</text>) +
          %(<text x="6" y="#{y + 16}" font-size="12" fill="var(--muted-foreground)">#{esc(total)}</text>)
      end

      def lane_total(money)
        return t(".none_open") if money.nil? || money.zero?

        t(".open_amount", amount: money.format(no_cents_if_whole: true))
      end

      def bars_svg
        bars.map { |bar| bar_svg(bar) }.join
      end

      def bar_svg(bar)
        o = bar[:obligation]
        y, height = bar[:up] ? [ AXIS_Y - bar[:h], bar[:h] ] : [ AXIS_Y, bar[:h] ]
        x = (bar[:x] - BAR_W / 2.0).round(1)
        rect = %(<rect x="#{x}" y="#{y.round(1)}" width="#{BAR_W}" height="#{height.round(1)}" rx="3" style="fill:#{bar[:fill]}"#{bar[:opacity]}/>)
        title = %(<title>#{esc(tooltip(o))}</title>)
        # A labelled bar carries its text (and inline glyph for late); every other
        # late bar still gets a small warning glyph at its tip, so lateness never
        # rides on colour alone.
        annotation = if bar[:label]
          bar_label(bar)
        elsif o.late?
          tip_warning(bar)
        else
          ""
        end
        %(<a href="#ob-#{esc(o.id)}">#{title}#{rect}</a>#{annotation})
      end

      # Text (and an inline warning glyph for late) beside the two largest bars per
      # lane; the rest carry only the <title> tooltip (and a tip glyph when late).
      def bar_label(bar)
        o = bar[:obligation]
        anchor = bar[:x] > W * 0.72 ? "end" : "start"
        dx = anchor == "end" ? -8 : 8
        ty = bar[:up] ? (AXIS_Y - bar[:h] - 8) : (AXIS_Y + bar[:h] + 16)
        text = esc(label_text(o))
        glyph = o.late? ? warning_glyph(bar[:x] + (anchor == "end" ? -label_width(o) - dx : dx), ty) : ""
        gx = o.late? && anchor == "start" ? bar[:x] + dx + 16 : bar[:x] + dx
        glyph +
          %(<text x="#{gx.round(1)}" y="#{ty}" text-anchor="#{anchor}" font-size="11.5" fill="var(--muted-foreground)">#{text}</text>)
      end

      def warning_glyph(x, y)
        %(<g transform="translate(#{(x).round(1)}, #{(y - 11).round(1)}) scale(0.55)" fill="none" stroke="var(--destructive)" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round">#{Glyphs::ICONS[:warning]}</g>)
      end

      # A small centred warning glyph just past an unlabelled late bar's tip.
      def tip_warning(bar)
        gx = (bar[:x] - 6).round(1)
        gy = bar[:up] ? (AXIS_Y - bar[:h] - 15) : (AXIS_Y + bar[:h] + 3)
        %(<g transform="translate(#{gx}, #{gy.round(1)}) scale(0.5)" fill="none" stroke="var(--destructive)" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round">#{Glyphs::ICONS[:warning]}</g>)
      end

      # Rough advance for right-anchored glyph offset (monospace-ish estimate).
      def label_width(o) = label_text(o).length * 6

      def label_text(o)
        amount = o.amount&.format(no_cents_if_whole: true)
        "#{o.counterpart} · #{amount} · #{label_detail(o)}"
      end

      def label_detail(o)
        case o.status
        when :late    then t("money.status.late", count: o.days_late(@today))
        when :settled then t("money.status.paid", date: l(o.settled_on, format: :day_month))
        when :decide  then t(".renews", date: l(o.due_on, format: :day_month))
        else               t(".due", date: l(o.due_on, format: :day_month))
        end
      end

      def tooltip(o)
        "#{o.counterpart} · #{o.what} · #{o.amount&.format} · #{label_detail(o)}"
      end

      # ── Overflow markers ─────────────────────────────────────────────────────
      # Dashed boundary + axis caption between the live window and each gutter.
      def overflow_dividers
        out = +""
        if backlog_before.any?
          out << dashed_rule(axis_left)
          out << edge_caption(PLOT_LEFT + 12, t(".overflow.older_axis"), "start")
        end
        if tail_after.any?
          out << dashed_rule(axis_right)
          out << edge_caption(PLOT_RIGHT - 12, t(".overflow.later_axis"), "end")
        end
        out
      end

      def dashed_rule(x)
        %(<line x1="#{x}" y1="26" x2="#{x}" y2="226" stroke="var(--border)" stroke-dasharray="3 4" stroke-width="1"/>)
      end

      def edge_caption(x, text, anchor)
        %(<text x="#{x}" y="#{TICK_Y}" text-anchor="#{anchor}" font-family="JetBrains Mono, monospace" font-size="10.5" fill="var(--muted-foreground)">#{esc(text)}</text>)
      end

      def overflow_markers
        out = +""
        [ true, false ].each do |up|
          out << overflow_marker(backlog_before, up, :older, PLOT_LEFT + 12) if backlog_before.any?
          out << overflow_marker(tail_after, up, :later, axis_right + 12) if tail_after.any?
        end
        out
      end

      # One lane's overflow, read like a lane total: a stacked-bars glyph, then the
      # count and the summed amount. Links to the ledger, where the full list lives.
      def overflow_marker(list, up, kind, gx)
        data = overflow_data(list, up)
        return "" unless data

        cy = MARKER_Y[up ? :up : :down]
        tx = gx + 32
        count = helpers.number_with_delimiter(data[:count])
        fill = kind == :older ? "var(--destructive)" : "var(--foreground)"
        %(<a href="#money_ledger">) +
          %(<title>#{esc(t(".overflow.#{kind}_title", count: count, amount: data[:money].format))}</title>) +
          stacked_glyph(gx, cy + 8, fill) +
          %(<text x="#{tx}" y="#{cy - 1}" font-size="12.5" font-weight="650" fill="var(--foreground)">#{esc(t(".overflow.#{kind}", count: count))}</text>) +
          %(<text x="#{tx}" y="#{cy + 14}" font-size="11" fill="var(--muted-foreground)">#{esc(data[:money].format(no_cents_if_whole: true))}</text>) +
          "</a>"
      end

      def stacked_glyph(gx, base, fill)
        [ [ 0, 10 ], [ 6, 16 ], [ 12, 12 ], [ 18, 20 ] ].map do |dx, h|
          %(<rect x="#{gx + dx}" y="#{base - h}" width="4" height="#{h}" rx="1.5" fill="#{fill}"/>)
        end.join
      end

      # Count + summed amount for one lane of an overflow list. The amount collapses
      # to the primary currency (or, absent it, the largest), mirroring lane totals.
      def overflow_data(list, up)
        group = list.select { |o| o.receivable? == up }
        return nil if group.empty?

        by_currency = group.each_with_object(Hash.new(0)) { |o, acc| acc[o.currency] += o.amount_cents.to_i.abs }
        primary = @summary.primary_currency
        currency = by_currency.key?(primary) ? primary : by_currency.max_by { |_c, cents| cents }.first
        { count: group.size, money: ::Money.new(by_currency[currency], currency) }
      end

      # ── Geometry ───────────────────────────────────────────────────────────
      # Obligations that fall inside the axis window (drawn as bars) vs. those that
      # spill before it (older backlog) or after it (later, unsettled due dates).
      def plotted
        @plotted ||= @ledger.obligations.select do |o|
          (date = bar_date(o)) && date >= @ledger.range_start && date <= @ledger.range_end
        end
      end

      def backlog_before
        @backlog_before ||= @ledger.obligations.select do |o|
          !o.settled? && (date = bar_date(o)) && date < @ledger.range_start
        end
      end

      def tail_after
        @tail_after ||= @ledger.obligations.select do |o|
          !o.settled? && (date = bar_date(o)) && date > @ledger.range_end
        end
      end

      def bars
        @bars ||= begin
          max = plotted.filter_map { |o| o.amount_cents&.abs }.max.to_i
          labelled = labelled_ids
          plotted.map do |o|
            {
              obligation: o, x: position(bar_date(o)), up: o.receivable?, h: bar_height(o, max),
              fill: fill_for(o), opacity: (o.settled? ? %( fill-opacity="0.5") : ""),
              label: labelled.include?(o.id)
            }
          end
        end
      end

      # Label the two largest bars per lane, then drop any that would overprint a
      # kept one (left-to-right). Everything else keeps its tooltip (and tip glyph).
      def labelled_ids
        @labelled_ids ||= begin
          candidates = plotted.group_by(&:receivable?).flat_map do |_receivable, group|
            group.max_by(2) { |o| o.amount_cents.to_i.abs }
          end.map(&:id).to_set

          kept = Set.new
          bars_for(candidates).group_by { |b| b[:up] }.each_value do |group|
            occupied = []
            # Largest first, so the more important bar keeps its label when two collide.
            group.sort_by { |b| -b[:obligation].amount_cents.to_i.abs }.each do |b|
              range = label_range(b)
              next if occupied.any? { |lo, hi| range.first < hi + 8 && range.last > lo - 8 }

              occupied << range
              kept << b[:obligation].id
            end
          end
          kept
        end
      end

      # Bar geometry for the label candidates, without the label flag (used only to
      # place labels, so it must not call #bars — which asks for #labelled_ids).
      def bars_for(ids)
        max = plotted.filter_map { |o| o.amount_cents&.abs }.max.to_i
        plotted.select { |o| ids.include?(o.id) }
               .map { |o| { obligation: o, x: position(bar_date(o)), up: o.receivable?, h: bar_height(o, max) } }
      end

      def label_range(bar)
        w = label_width(bar[:obligation])
        anchor_end = bar[:x] > W * 0.72
        x1 = anchor_end ? bar[:x] - w : bar[:x]
        [ x1, x1 + w ]
      end

      def bar_date(o)
        (o.settled? ? o.settled_on : o.due_on)
      end

      def bar_height(o, max)
        cents = o.amount_cents.to_i.abs
        return MIN_BAR if cents.zero? || max.zero?

        [ [ Math.sqrt(cents.to_f / max) * MAX_BAR, MIN_BAR ].max, MAX_BAR ].min
      end

      def fill_for(o)
        return "var(--destructive)" if o.late?
        return "var(--muted-foreground)" if o.settled?

        "var(--foreground)"
      end

      # The live axis contracts to leave a gutter for whichever side overflows; with
      # no overflow it is the full PLOT_LEFT..PLOT_RIGHT span (identical to before).
      def axis_left
        @axis_left ||= PLOT_LEFT + (backlog_before.any? ? GUTTER : 0)
      end

      def axis_right
        @axis_right ||= PLOT_RIGHT - (tail_after.any? ? TAIL : 0)
      end

      def position(date)
        span = (@ledger.range_end - @ledger.range_start).to_f
        span = 1 if span.zero?
        ratio = (date.to_date - @ledger.range_start).to_f / span
        x = axis_left + ratio.clamp(0.0, 1.0) * (axis_right - axis_left)
        x.round(2)
      end

      def ticks
        out = []
        date = @ledger.range_start
        while date <= @ledger.range_end
          out << date
          date += 7
        end
        out
      end

      def horizon_days
        (@ledger.range_end - @today).to_i
      end

      def esc(str) = ERB::Util.html_escape(str.to_s)
    end
  end
end
