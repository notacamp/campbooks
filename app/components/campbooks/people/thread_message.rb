# frozen_string_literal: true

module Campbooks
  module People
    # One message row inside a ThreadBlock. Uses a native <details> element so no
    # JavaScript is needed: the newest message gets the `open` attribute, older
    # ones start folded.
    #
    # The summary (always-visible) line carries:
    #   - a small avatar
    #   - the sender name ("You" for sent, else display name or address local-part)
    #   - when CLOSED: a one-line snippet of the body
    #   - when OPEN: the from address (hidden below sm), a "to you" / "to <name>" label,
    #     a paperclip glyph when attachments are present, and the received-at time
    #
    # The content (shown when open) is the sanitized body plus an attachments row.
    # We use Tailwind's `group` on <details> so child elements can react to
    # open/closed state via `group-open:*` utilities.
    #
    # @param message [EmailMessage]
    # @param person_first_name [String] e.g. "Sofia"
    # @param open [Boolean] whether this <details> starts open
    # @param full_body [Boolean] render the full body (newest of newest thread only)
    class ThreadMessage < Campbooks::Base
      CLIP_ICON = '<svg class="h-3.5 w-3.5 flex-shrink-0 text-muted-foreground" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15.172 7l-6.586 6.586a2 2 0 102.828 2.828l6.414-6.586a4 4 0 00-5.656-5.656l-6.415 6.585a6 6 0 108.486 8.486L20.5 13"/></svg>'
      PREVIEW_THRESHOLD = 600

      def initialize(message:, person_first_name:, open: false, full_body: false)
        @message = message
        @person_first_name = person_first_name
        @open = open
        @full_body = full_body
      end

      def view_template
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
          div(class: "mt-2 overflow-x-auto") { message_body }
          attachments_row if has_attachments?
        end
      end

      private

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
