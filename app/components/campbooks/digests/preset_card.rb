# frozen_string_literal: true

module Campbooks
  module Digests
    # One card in the preset gallery (/digests/new without a preset param).
    # Clicking navigates to /digests/new?preset=<key> which pre-fills the form.
    class PresetCard < Campbooks::Base
      ICONS = {
        "inbox"     => '<path d="M22 12h-6l-2 3h-4l-2-3H2"/><path d="M5.45 5.11L2 12v6a2 2 0 002 2h16a2 2 0 002-2v-6l-3.45-6.89A2 2 0 0016.76 4H7.24a2 2 0 00-1.79 1.11z"/>',
        "calendar"  => '<rect x="3" y="4.5" width="18" height="16.5" rx="2"/><path d="M3 9.5h18M8 3v4M16 3v4"/>',
        "tasks"     => '<path d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2"/><path d="M9 5a2 2 0 012-2h2a2 2 0 012 2"/><path d="m9 14 2 2 4-4"/>',
        "documents" => '<rect x="5" y="3" width="14" height="18" rx="2"/><path d="M9 8h6M9 12h6M9 16h3"/>',
        "custom"    => '<circle cx="12" cy="12" r="3"/><path d="M19.07 4.93a10 10 0 010 14.14"/><path d="M4.93 4.93a10 10 0 000 14.14"/>'
      }.freeze

      def initialize(preset:)
        @preset = preset
      end

      def view_template
        a(
          href: helpers.new_digest_path(preset: @preset.key),
          class: "group flex flex-col gap-3 rounded-2xl border border-border bg-card p-5 transition-all hover:border-foreground/30 hover:shadow-md"
        ) do
          div(class: "flex items-center gap-3") do
            div(class: "flex h-10 w-10 flex-shrink-0 items-center justify-center rounded-xl bg-muted text-foreground/70 transition-colors group-hover:bg-foreground/10") do
              icon_svg
            end
            p(class: "truncate text-sm font-semibold text-foreground") do
              t("digests.presets.#{@preset.key}.label")
            end
          end
          p(class: "text-[13px] leading-relaxed text-muted-foreground") do
            t("digests.presets.#{@preset.key}.description")
          end
          cadence_hint
        end
      end

      private

      def icon_svg
        path = ICONS[@preset.icon] || ICONS["custom"]
        raw safe(%(<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">#{path}</svg>))
      end

      def cadence_hint
        hint = @preset.schedule_hint
        day_name = hint[:wday] ? helpers.t("date.day_names")[hint[:wday]] : nil
        # Kernel#format is shadowed by Phlex — use String#% instead.
        time_str = "%02d:%02d" % [ hint[:hour], hint[:min] ]
        label = day_name ? "#{day_name} #{time_str}" : time_str
        span(class: "self-start inline-flex items-center rounded-full bg-muted px-2.5 py-0.5 text-[11px] font-medium text-muted-foreground") do
          plain label
        end
      end
    end
  end
end
