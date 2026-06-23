# frozen_string_literal: true

module Campbooks
  # One big content card in the home feed: the email itself (sender, subject,
  # excerpt), a meta row (#tag, attachments, thread count, priority accent),
  # Scout's read, and the suggested actions — all actionable inline. The feed
  # container spaces these so one card dominates the viewport.
  class FeedCard < Campbooks::Base
    # category → soft status tone, so #tags carry a subtle colour
    TAG_TONES = {
      "invoice" => "tone-amber", "payment" => "tone-amber", "receipt" => "tone-green",
      "parent" => "tone-blue", "registration" => "tone-blue", "vendor" => "tone-violet",
      "team" => "tone-neutral", "priority" => "tone-orange", "permit" => "tone-orange"
    }.freeze

    def initialize(initials:, sender:, time:, subject:, excerpt:, scout:, prime:,
                   tag: nil, attachment: nil, thread_count: nil, priority: false, href: nil, **attrs)
      @href = href
      @initials = initials
      @sender = sender
      @time = time
      @subject = subject
      @excerpt = excerpt
      @scout = scout
      @prime = prime
      @tag = tag
      @attachment = attachment
      @thread_count = thread_count
      @priority = priority
      @attrs = attrs
    end

    def view_template
      custom = @attrs.delete(:class)
      div(class: class_names("rounded-[22px] border border-border bg-card p-6", custom), **@attrs) do
        div(class: "flex items-center gap-3") do
          span(class: "flex h-[38px] w-[38px] flex-shrink-0 items-center justify-center rounded-full bg-secondary text-[13px] font-semibold text-secondary-foreground") { @initials }
          div(class: "min-w-0 flex-1") do
            div(class: "truncate text-[13px] font-semibold text-foreground") { @sender }
            div(class: "text-[11.5px] text-muted-foreground") { @time }
          end
        end

        h3(class: "mt-3.5 text-[17px] font-semibold leading-snug tracking-tight text-foreground") { @subject }
        p(class: "mt-2 text-sm leading-relaxed text-muted-foreground") { @excerpt }

        meta_row

        render Campbooks::ScoutNote.new(message: @scout, time: "read it just now", class: "mt-[18px]")

        div(class: "mt-4 flex flex-wrap justify-end gap-2") do
          link = @href ? { href: @href } : {}
          render Campbooks::Button.new(variant: :ghost, size: :sm, **link) { "Archive" }
          render Campbooks::Button.new(variant: :ghost, size: :sm, **link) { "Reply" }
          render Campbooks::Button.new(variant: :primary, size: :sm, **link) { @prime }
        end
      end
    end

    private

    def meta_row
      return unless @tag || @attachment || thread_badge? || @priority

      div(class: "mt-3.5 flex flex-wrap gap-1.5") do
        tag_chip if @tag
        chip(clip_icon, @attachment) if @attachment
        chip(msg_icon, "#{@thread_count} messages") if thread_badge?
        priority_chip if @priority
      end
    end

    def thread_badge? = @thread_count.to_i > 1

    def chip(icon_svg, label)
      span(class: "inline-flex max-w-[210px] items-center gap-1.5 rounded-lg border border-border bg-muted px-2.5 py-1 text-[11.5px] font-medium text-foreground/80") do
        raw safe(icon_svg)
        span(class: "truncate") { label }
      end
    end

    def tag_chip
      tone = TAG_TONES[@tag.to_s.downcase] || "tone-neutral"
      span(class: class_names("inline-flex items-center rounded-lg px-2.5 py-1 text-[11.5px] font-semibold", tone)) { "##{@tag}" }
    end

    def priority_chip
      span(class: "scout-glass inline-flex items-center gap-1.5 rounded-lg px-2.5 py-1 text-[11.5px] font-semibold", style: "color: var(--ember-solid)") do
        span(class: "h-1.5 w-1.5 rounded-full", style: "background-color: var(--ember-solid)")
        plain "Priority"
      end
    end

    def clip_icon
      %(<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="h-3 w-3 text-muted-foreground"><path d="M21 9 12 18a4 4 0 0 1-6-6l8-8a3 3 0 0 1 4 4l-8.5 8.5"/></svg>)
    end

    def msg_icon
      %(<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="h-3 w-3 text-muted-foreground"><path d="M21 15a2 2 0 0 1-2 2H8l-4 3V6a2 2 0 0 1 2-2h13a2 2 0 0 1 2 2z"/></svg>)
    end
  end
end
