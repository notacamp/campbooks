# frozen_string_literal: true

module Campbooks
  # The Skim tray: a row of time-bucket rings (plus a leading "Skim all"). Tapping
  # a ring opens the Skim viewer at that bucket; "Skim all" runs every ring in
  # sequence. Renders nothing when there is nothing to skim.
  #
  # The horizontal scroller lives on the MOUNT (the inbox's skim_tray frame, the
  # home rings row), not here — so home can scroll the inbox rings and the
  # "Documents" ring together as one strip. This component is just the ring group.
  #
  # `doc_count` (home only) is the number of documents awaiting review — its skim
  # steps fold into the "Skim all" badge, and "Skim all" then chains the inbox
  # walk into document review (the `skim-all` controller). It is 0 on the inbox,
  # where "Skim all" is mail-only.
  class SkimTray < Campbooks::Base
    def initialize(rings:, doc_count: 0, **attrs)
      @rings = rings || []
      @doc_count = doc_count.to_i
      @attrs = attrs
    end

    def view_template
      return if @rings.empty? && @doc_count.zero?

      custom = @attrs.delete(:class)
      div(
        class: class_names("flex items-start gap-2.5", custom),
        **@attrs
      ) do
        render Campbooks::SkimRing.new(
          theme: nil, label: "Skim all", count: skim_all_count,
          data: { action: skim_all_action }
        )
        @rings.each do |ring|
          render Campbooks::SkimRing.new(
            theme: ring[:theme],
            label: ring[:label],
            count: ring[:count],
            # .to_s so the theme survives as "follow_ups" (a symbol param is dasherized
            # to "follow-ups", which then fails SkimStack's start-frame theme match).
            data: { action: "click->skim-overlay#openTo", skim_overlay_theme_param: ring[:theme].to_s }
          )
        end
      end
    end

    private

    # "Skim all" walks every skim STEP — across the inbox rings AND the document
    # review queue — so its badge sums both. Each ring's `count` is already its
    # step count (one cluster = one stack), and one document is one step.
    def skim_all_count
      @rings.sum { |ring| ring[:count].to_i } + @doc_count
    end

    # With inbox rings present, "Skim all" opens the inbox skim and ARMS the
    # seamless hand-off into documents — when the inbox stacks are done, the
    # `skim-all` controller continues straight into review (no extra tap). On the
    # inbox page there's no `skim-all` controller, so `#arm` is a harmless no-op
    # and this stays mail-only. With only documents to skim, it goes straight in.
    def skim_all_action
      if @rings.any?
        "click->skim-overlay#open click->skim-all#arm"
      else
        "click->doc-skim-overlay#open"
      end
    end
  end
end
