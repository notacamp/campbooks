# frozen_string_literal: true

module Campbooks
  module Money
    # The 30-day timeline: two directions on one axis. Owed to you above the line,
    # you owe below; the next N days ahead and the overdue to the left of today.
    # Every bar is one obligation at its due date (settled ones at the date the bank
    # cleared them), linked to its ledger row. Pure inline SVG — no JS library — that
    # scrolls horizontally on a phone inside its own container.
    #
    # Colour follows status (ink / destructive / muted), but every late marker also
    # carries a warning glyph and every labelled bar its text, so colour is never the
    # only signal (validated for protanopia in the mock).
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
          shaded_overdue + grid + axis + today_marker + lane_labels + bars_svg +
          "</svg>"
      end

      def shaded_overdue
        x = position(@today)
        %(<rect x="#{PLOT_LEFT}" y="20" width="#{(x - PLOT_LEFT).round(1)}" height="210" rx="8" fill="var(--secondary)" opacity="0.6"/>)
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
        label = bar[:label] ? bar_label(bar) : ""
        %(<a href="#ob-#{esc(o.id)}">#{title}#{rect}</a>#{label})
      end

      # Text (and a warning glyph for late) beside the two largest per lane + every
      # late bar; the rest carry only the <title> tooltip.
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

      # ── Geometry ───────────────────────────────────────────────────────────
      def bars
        @bars ||= begin
          max = @ledger.obligations.filter_map { |o| o.amount_cents&.abs }.max.to_i
          labelled = labelled_ids
          @ledger.obligations.filter_map do |o|
            date = bar_date(o)
            next unless date

            {
              obligation: o, x: position(date), up: o.receivable?, h: bar_height(o, max),
              fill: fill_for(o), opacity: (o.settled? ? %( fill-opacity="0.5") : ""),
              label: labelled.include?(o.id)
            }
          end
        end
      end

      def labelled_ids
        ids = @ledger.obligations.select(&:late?).map(&:id)
        @ledger.obligations.group_by(&:receivable?).each_value do |group|
          ids += group.max_by(2) { |o| o.amount_cents.to_i }.map(&:id)
        end
        ids.to_set
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

      def position(date)
        span = (@ledger.range_end - @ledger.range_start).to_f
        span = 1 if span.zero?
        ratio = (date.to_date - @ledger.range_start).to_f / span
        x = PLOT_LEFT + ratio.clamp(0.0, 1.0) * (PLOT_RIGHT - PLOT_LEFT)
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
