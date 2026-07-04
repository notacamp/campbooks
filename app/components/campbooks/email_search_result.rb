# frozen_string_literal: true

module Campbooks
  # A single flat message row in the inbox search results. Unlike the thread-list
  # rows this is message-level (search returns messages, not threads) and richer:
  # sender, subject, snippet, date, tags, attachment + category indicators. Clicking
  # opens the message in the full inbox shell (data-turbo-frame="_top"), matching
  # how thread rows navigate. The active search/filter params are carried on the
  # link (`link_params`) so opening a result keeps the filter alive — the show
  # action re-renders the same filtered list rather than resetting to the inbox.
  class EmailSearchResult < Campbooks::Base
    ATTACHMENT_ICON = '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" ' \
      'd="M15.172 7l-6.586 6.586a2 2 0 102.828 2.828l6.414-6.586a4 4 0 00-5.656-5.656l-6.415 6.585a6 6 0 108.486 8.486L20.5 13"/>'

    def initialize(message:, link_params: {}, active: false, **attrs)
      @message = message
      @link_params = link_params
      @active = active
      @attrs = attrs
    end

    def view_template
      a(
        href: helpers.email_message_path(@message, **link_href_params),
        class: class_names(
          "flex items-start gap-2.5 mx-1.5 rounded-xl px-2.5 py-2 transition-colors",
          # In-place navigation keeps focus on the clicked link — style the ring
          # instead of letting the UA outline linger (cf. _thread_row).
          "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-accent-500",
          @active ? "bg-subtle" : "hover:bg-muted",
          @attrs.delete(:class)
        ),
        data: { turbo_frame: "_top" },
        **@attrs
      ) do
        avatar
        body
      end
    end

    private

    # folder_id: "all" keeps the show action from redirecting to a folder context
    # (which would drop the query string); the active filter params ride alongside.
    # `page` is dropped so the reopened list starts at the top of the filtered set,
    # not whichever infinite-scroll page the clicked result happened to come from.
    def link_href_params
      { folder_id: "all" }.merge(@link_params.to_h.symbolize_keys.except(:page))
    end

    def avatar
      div(class: "relative shrink-0 mt-px") do
        render(Campbooks::ContactAvatar.new(
          email: avatar_email,
          sent: @message.sent?,
          size: :lg,
          contact_id: @message.contact_id,
          variant: :neutral,
          show_direction: true,
          account_color: @message.email_account&.color
        ))
        unless @message.read?
          span(class: "absolute -top-0.5 -right-0.5 w-2.5 h-2.5 rounded-full bg-blue-500 border-2 border-white dark:border-card")
        end
      end
    end

    def body
      div(class: "min-w-0 flex-1") do
        # Subject + date
        div(class: "flex items-start justify-between gap-1") do
          span(class: class_names("text-gray-900 truncate text-[11px] leading-tight", ("font-bold" unless @message.read?))) do
            @message.subject.presence || t(".no_subject")
          end
          if @message.received_at
            span(class: "text-gray-400 flex-shrink-0 text-[10px] leading-tight") { l(@message.received_at, format: :full) }
          end
        end

        # Sender + attachment indicator
        div(class: "flex items-center gap-1.5 mt-0.5") do
          span(class: "text-gray-500 truncate text-[10px]") { sender_label }
          attachment_indicator if @message.has_attachment?
        end

        # Snippet
        if snippet.present?
          p(class: "mt-0.5 text-[10px] leading-snug text-gray-400 line-clamp-2") { snippet }
        end

        # Tags + category
        if tags.any? || category_chip?
          div(class: "mt-1 flex items-center flex-wrap gap-1") do
            tags.each { |tag| render(Campbooks::TagChip.new(tag: tag, size: :sm)) }
            if category_chip?
              render(Campbooks::CategoryChip.new(category: @message.category.to_sym, size: :sm, label: false))
            end
          end
        end
      end
    end

    def attachment_indicator
      span(class: "inline-flex items-center justify-center flex-shrink-0 w-4 h-4 rounded-full bg-gray-200") do
        svg(class: "w-2 h-2 text-gray-500", fill: "none", stroke: "currentColor", viewBox: "0 0 24 24", aria_hidden: "true") do
          raw(safe(ATTACHMENT_ICON))
        end
      end
    end

    def avatar_email
      (@message.sent? ? @message.to_address.presence : @message.from_address) || "?"
    end

    def sender_label
      if @message.sent?
        t(".sent_to", address: clean_email_address(@message.to_address) || @message.to_address)
      else
        clean_email_address(@message.from_address) || @message.from_address
      end
    end

    def snippet
      @snippet ||= (@message.ai_summary.presence || helpers.strip_tags(@message.body.to_s)).to_s.squish.truncate(160)
    end

    def tags
      # Hide provider system / AI low-value labels. Filtered in Ruby (not via
      # visible_for's query) so the component's struct-based Lookbook preview,
      # whose tags don't respond to #hidden?, still renders.
      @tags ||= @message.tags.to_a.reject { |t| t.try(:hidden?) }.first(3)
    end

    def category_chip?
      @message.category.present? && Campbooks::CategoryChip::CATEGORIES.include?(@message.category.to_sym)
    end
  end
end
