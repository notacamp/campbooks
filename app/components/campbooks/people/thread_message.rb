# frozen_string_literal: true

module Campbooks
  module People
    # One message row inside a ThreadBlock. Uses a native <details> element so no
    # JavaScript is needed: the newest message gets the `open` attribute, older
    # ones start folded beneath it.
    #
    # The summary (always-visible) line carries:
    #   - a small avatar
    #   - the sender name ("You" for sent, else display name or address local-part)
    #   - when CLOSED: a one-line snippet of the body
    #   - when OPEN: the from address (hidden below sm), a "to you" / "to <name>" label,
    #     a paperclip glyph when attachments are present, and the received-at time
    #
    # The content (shown when open) is the sanitized body plus an attachments row.
    # When `can_send` is true an actions row (Reply / Reply all / Forward) appears
    # after the body so every open message has its own compose buttons.
    # With `lazy_src` the content is a lazy turbo-frame instead, so a folded
    # message costs nothing until it is expanded: the frame loads the body from
    # People::MessagesController, which renders this component `content_only`.
    # We use Tailwind's `group` on <details> so child elements can react to
    # open/closed state via `group-open:*` utilities.
    #
    # @param message [EmailMessage]
    # @param person_first_name [String] e.g. "Sofia"
    # @param open [Boolean] whether this <details> starts open
    # @param full_body [Boolean] render the full body (newest of newest thread only)
    # @param lazy_src [String, nil] URL that serves the body (folded messages)
    # @param content_only [Boolean] render just the body + attachments (the frame's content)
    # @param can_send [Boolean] when true, render Reply / Reply all / Forward buttons
    class ThreadMessage < Campbooks::Base
      register_element :turbo_frame

      CLIP_ICON = '<svg class="h-3.5 w-3.5 flex-shrink-0 text-muted-foreground" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15.172 7l-6.586 6.586a2 2 0 102.828 2.828l6.414-6.586a4 4 0 00-5.656-5.656l-6.415 6.585a6 6 0 108.486 8.486L20.5 13"/></svg>'
      REPLY_ICON = '<svg class="h-3.5 w-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 10h10a8 8 0 018 8v2M3 10l6 6m-6-6l6-6"/></svg>'
      REPLY_ALL_ICON = '<svg class="h-3.5 w-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 10H3l6-6m-6 6l6 6M17 10h-7m7 0c2.761 0 5 2.239 5 5v3"/></svg>'
      FORWARD_ICON = '<svg class="h-3.5 w-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 10H11a8 8 0 00-8 8v2m18-10l-6-6m6 6l-6 6"/></svg>'
      PREVIEW_THRESHOLD = 600

      def initialize(message:, person_first_name:, open: false, full_body: false, lazy_src: nil,
                     content_only: false, can_send: false)
        @message = message
        @person_first_name = person_first_name
        @open = open
        @full_body = full_body
        @lazy_src = lazy_src
        @content_only = content_only
        @can_send = can_send
      end

      def view_template
        return message_content if @content_only

        details(class: "group px-4 py-3", open: @open || nil) do
          summary(class: "flex min-w-0 cursor-pointer list-none items-center gap-2") do
            render(ContactAvatar.new(
              email: avatar_email,
              sent: @message.sent?,
              size: :sm,
              contact_id: @message.contact_id,
              variant: :neutral,
              show_direction: false
            ))
            span(class: "min-w-0 truncate text-[13px] font-semibold text-foreground") { plain(sender_name) }
            # Snippet: visible when closed, hidden when open (group-open:hidden)
            span(class: "min-w-0 flex-1 truncate text-[12px] text-muted-foreground group-open:hidden") { plain(snippet) }
            # Address + to label + clip — hidden when closed, shown when open
            div(class: "hidden min-w-0 items-center gap-2 group-open:flex") do
              span(class: "hidden max-w-[200px] truncate text-[11.5px] text-muted-foreground sm:inline") { plain(@message.from_address.to_s) }
              span(class: "flex-shrink-0 text-[11.5px] text-muted-foreground") { plain(to_label) }
              raw(safe(CLIP_ICON)) if has_attachments?
            end
            span(class: "flex-shrink-0 text-[11.5px] text-muted-foreground") do
              plain(@message.received_at ? l(@message.received_at, format: :at_short) : "")
            end
          end
          if @lazy_src
            lazy_content
          else
            message_content
          end
        end
      end

      private

      # A closed <details> keeps its frame out of view, so Turbo only fetches the
      # body once the row is expanded.
      def lazy_content
        turbo_frame(id: "people_message_#{@message.id}", src: @lazy_src, loading: "lazy", class: "mt-2 block") do
          div(class: "flex items-center py-1") { render(Campbooks::Spinner.new(size: :sm)) }
        end
      end

      def message_content
        div(class: @content_only ? "overflow-x-auto" : "mt-2 overflow-x-auto") { message_body }
        attachments_row if has_attachments?
        actions_row if @can_send
      end

      def actions_row
        div(class: "mt-2.5 flex flex-wrap items-center gap-1.5") do
          compose_form_button(:reply,     REPLY_ICON,     t(".reply"),     "people_reply")
          compose_form_button(:reply_all, REPLY_ALL_ICON, t(".reply_all"), "people_reply_all")
          compose_form_button(:forward,   FORWARD_ICON,   t(".forward"),   "people_forward")
        end
      end

      def compose_form_button(mode, icon_svg, label, data_key)
        short_key = data_key.delete_prefix("people_")
        hint_key  = ".#{short_key}_hint"
        hint_label = t(hint_key, default: label)
        key_map = { "reply" => "r", "reply_all" => "a", "forward" => "f" }
        key = key_map[short_key]
        # Use a symbol so Phlex dasherizes the data attribute (people_reply -> data-people-reply)
        form(action: helpers.compose_email_message_path(@message, mode: mode), method: "post", class: "inline-flex") do
          input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
          button(type: "submit",
                 data: { data_key.to_sym => true, **hint_data(hint_label, key: key) },
                 aria: hint_aria(key),
                 class: "inline-flex cursor-pointer items-center gap-1.5 rounded-lg border border-border px-2.5 py-1 text-[12px] font-medium text-foreground hover:bg-secondary") do
            raw(safe(icon_svg)); whitespace; plain(label)
          end
        end
      end

      def avatar_email
        @message.sent? ? (@message.email_account&.email_address || "?") : (@message.from_address || "?")
      end

      def sender_name
        if @message.sent?
          t("people.conversation.you")
        else
          @message.contact&.display_name.presence ||
            @message.from_address.to_s.split("@").first.to_s.tr(".", " ").titleize
        end
      end

      def snippet
        (@message.summary.presence || helpers.strip_tags(@message.body.to_s).squish).truncate(110)
      end

      def to_label
        @message.sent? ? t(".to_name", name: @person_first_name) : t(".to_you")
      end

      def has_attachments?
        @message.files.attached?
      end

      def message_body
        if @message.body.blank? && @message.summary.blank?
          div(class: "text-[13px] italic text-muted-foreground") { plain(t("components.message_bubble.no_content")) }
        elsif @full_body
          div(class: "text-sm leading-relaxed text-foreground/90", style: "word-wrap:break-word") do
            raw(safe(linkify_mentions(safe_email_body_full(@message))))
          end
        else
          div(class: "text-sm leading-relaxed text-foreground/90", style: "word-wrap:break-word") do
            raw(safe(email_preview_html(@message)))
          end
          show_full_toggle if long_body?
        end
      end

      def long_body?
        helpers.strip_tags(@message.body.to_s).length > PREVIEW_THRESHOLD
      end

      def show_full_toggle
        details(class: "group/full mt-1.5") do
          summary(class: "inline-flex cursor-pointer select-none list-none items-center gap-1 text-[12px] font-medium text-muted-foreground hover:text-foreground") do
            span(class: "group-open/full:hidden") { plain(t("components.message_bubble.show_full")) }
            span(class: "hidden group-open/full:inline") { plain(t("components.message_bubble.hide_full")) }
          end
          div(class: "mt-2 max-w-[66ch] overflow-x-auto text-sm leading-relaxed text-foreground/90", style: "word-wrap:break-word") do
            raw(safe(linkify_mentions(safe_email_body_full(@message))))
          end
        end
      end

      def attachments_row
        files = @message.files
        return if files.blank?

        div(class: "mt-2.5 flex flex-wrap items-center gap-2") do
          files.each do |file|
            a(href: helpers.rails_blob_path(file), target: "_blank", rel: "noopener",
              class: "inline-flex max-w-full items-center gap-1.5 rounded-lg bg-secondary px-2.5 py-1 text-[12px] text-foreground/80 hover:bg-secondary/70") do
              raw(safe(CLIP_ICON))
              span(class: "max-w-[220px] truncate") { plain(file.filename.to_s) }
              if file.blob&.byte_size
                span(class: "text-muted-foreground") { plain("· #{helpers.number_to_human_size(file.blob.byte_size)}") }
              end
            end
          end
        end
      end
    end
  end
end
