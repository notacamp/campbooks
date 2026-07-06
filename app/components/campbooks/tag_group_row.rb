# frozen_string_literal: true

module Campbooks
  # One collapsed tag-group row in the inbox list (a default or a custom tag
  # group). Reads like a quiet thread row on the same gutter: a facepile of
  # the group's recent senders, the group's color dot + name, the collapsed
  # thread count, and a drill-in chevron. Identity is carried by the ColorDot
  # (the same vocabulary as document types in thread rows) — the row itself
  # stays on the canvas, never a tinted band.
  #
  # A checkbox appears on hover (or permanently in select mode), wired to the
  # email-selection controller so every bulk-action toolbar tool works on the
  # whole group without navigating into the drill-in view first.
  #
  #   render Campbooks::TagGroupRow.new(
  #     label: "Newsletters & promos", count: 3, color: "#d44996",
  #     href: email_messages_path(group: "Newsletters & promos"),
  #     senders: [{ email: "deals@shop.example", contact_id: 1 }]
  #   )
  class TagGroupRow < Campbooks::Base
    CHEVRON = '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7"/>'
    TAG_ICON = '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" ' \
               'd="M7 7h.01M7 3h5c.512 0 1.024.195 1.414.586l7 7a2 2 0 010 2.828l-7 7a2 2 0 ' \
               '01-2.828 0l-7-7A1.994 1.994 0 013 12V7a4 4 0 014-4z"/>'

    # @param label [String] the group's display name
    # @param count [Integer] how many threads are collapsed into the group
    # @param href [String] drill-in URL for the group's filtered list
    # @param senders [Array<Hash>] recent senders [{ email:, contact_id: }]
    # @param color [String, nil] the group's tag color (hex) for the identity dot
    def initialize(label:, count:, href:, senders: [], color: nil)
      @label = label
      @count = count
      @href = href
      @senders = Array(senders)
      @color = color
    end

    def view_template
      # Outer container — not a link, so clicking the checkbox does not navigate.
      # group/grouprow: named group for hover-reveal of chevron + checkbox.
      div(
        class: "group/grouprow flex items-center mx-1.5 rounded-xl transition-colors hover:bg-muted",
        data: { testid: "tag-group-row" }
      ) do
        checkbox_area
        drill_in_link
      end
    end

    private

    # Checkbox area — same animated w-0 → w-6 pattern as thread rows.
    # Hidden by default; revealed on hover or when select-mode is active on
    # the email-selection ancestor (group/select + data-select-mode="on").
    # Checked state persists the width (via [&:has(input:checked)]:w-6).
    def checkbox_area
      div(
        class: "flex-shrink-0 w-0 " \
               "group-hover/grouprow:w-6 " \
               "[&:has(input:checked)]:w-6 " \
               "group-data-[select-mode=on]/select:w-6 " \
               "overflow-hidden transition-all duration-150 flex items-center pl-1.5 self-stretch relative z-10"
      ) do
        input(
          type: "checkbox",
          class: "w-3.5 h-3.5 rounded border-gray-300 text-accent-600 focus:ring-accent-500 " \
                 "cursor-pointer " \
                 "opacity-0 group-hover/grouprow:opacity-100 checked:opacity-100 " \
                 "group-data-[select-mode=on]/select:opacity-100 " \
                 "transition-opacity bg-transparent",
          data: {
            action: "change->email-selection#toggleGroup",
            email_selection_target: "groupCheckbox",
            group_name: @label,
            group_count: @count
          },
          aria_label: t(".select_group_aria", label: @label)
        )
      end
    end

    # The drill-in link covers the rest of the row. Clicking the label, facepile,
    # or count navigates into the group's filtered view. The link is a flex child
    # of the outer div, not a full-row wrapper, so the checkbox above stays outside.
    def drill_in_link
      a(
        href: @href,
        class: "flex items-center flex-1 min-w-0 rounded-xl pr-3 " \
               "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-accent-500",
        style: "padding-top:var(--thread-py,0.625rem);padding-bottom:var(--thread-py,0.625rem);gap:var(--thread-gap,0.875rem)",
        aria_label: t(".aria_label", label: @label, count: @count),
        data: { turbo_frame: "_top" }
      ) do
        facepile
        div(class: "min-w-0 flex-1") do
          div(class: "flex items-center gap-1.5 min-w-0") do
            render(ColorDot.new(color: @color, size: :sm)) if @color.present?
            span(class: "text-gray-900 font-medium truncate", style: "font-size:var(--thread-subject-size,13px);line-height:1.4") { @label }
          end
          senders_line
        end
        span(class: "text-gray-400 font-medium tabular-nums flex-shrink-0", style: "font-size:var(--thread-meta-size,10px)") { @count.to_s }
        svg(class: "w-3.5 h-3.5 text-gray-300 group-hover/grouprow:text-gray-400 transition-colors flex-shrink-0",
            fill: "none", stroke: "currentColor", viewBox: "0 0 24 24", aria_hidden: "true") { raw(safe(CHEVRON)) }
      end
    end

    # Recent senders as the standard facepile (ring cutouts, "+N" overflow).
    # Sits on the thread rows' avatar gutter — min-w matches the single :lg
    # avatar — and widens for multiple faces exactly like a multi-sender
    # thread row does. Falls back to a neutral tag glyph for an empty group.
    def facepile
      div(class: "flex min-w-[38px] shrink-0 items-center", aria_hidden: "true") do
        if @senders.any?
          render ContactAvatarGroup.new(participants: @senders, size: :sm, max: 3, variant: :neutral)
        else
          span(class: "inline-flex h-6 w-6 items-center justify-center rounded-full bg-subtle text-muted-foreground") do
            svg(class: "w-3 h-3", fill: "none", stroke: "currentColor", viewBox: "0 0 24 24", aria_hidden: "true") { raw(safe(TAG_ICON)) }
          end
        end
      end
    end

    def senders_line
      return if @senders.empty?

      names = @senders.map { |s| (s[:email] || "?").to_s.split("@").first }
      text =
        case names.length
        when 1 then names.first
        when 2 then "#{names[0]}, #{names[1]}"
        else t("email_messages.thread_list.senders_more", first: names[0], second: names[1])
        end
      span(class: "text-gray-400 truncate block", style: "font-size:var(--thread-meta-size,10px)") { text }
    end
  end
end
