# frozen_string_literal: true

module Campbooks
  module People
    # One email thread in the Person conversation pane. Renders as:
    #   * a heading row (subject, message count + date, "Open in inbox" link)
    #   * each message as a <details> row (ThreadMessage), oldest closed, newest open
    #   * when this is the newest thread and the user may send: an inline reply area
    #     (Scout draft card when a draft exists; otherwise Reply / Reply all / Forward
    #     ghost buttons that all open the Compose Dock)
    #
    # @param conversation_thread [People::ConversationThread]
    # @param person_first_name [String]
    # @param newest_thread [Boolean] true for the first thread on page 1
    # @param can_send [Boolean]
    # @param scout_draft [String, nil] pre-fetched draft content (or nil)
    class ThreadBlock < Campbooks::Base
      CLIP_ICON = '<svg class="h-3.5 w-3.5 flex-shrink-0 text-muted-foreground" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15.172 7l-6.586 6.586a2 2 0 102.828 2.828l6.414-6.586a4 4 0 00-5.656-5.656l-6.415 6.585a6 6 0 108.486 8.486L20.5 13"/></svg>'
      SPARK_ICON = '<svg viewBox="0 0 24 24" fill="currentColor" class="h-3 w-3 flex-shrink-0" style="color:var(--ember-solid)" aria-hidden="true"><path d="M12 5l1.7 5.6L19.5 12l-5.8 1.4L12 19l-1.7-5.6L4.5 12l5.8-1.4z"/></svg>'
      REPLY_ICON = '<svg class="h-3.5 w-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 10h10a8 8 0 018 8v2M3 10l6 6m-6-6l6-6"/></svg>'
      REPLY_ALL_ICON = '<svg class="h-3.5 w-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 10H3l6-6m-6 6l6 6M17 10h-7m7 0c2.761 0 5 2.239 5 5v3"/></svg>'
      FORWARD_ICON = '<svg class="h-3.5 w-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 10H11a8 8 0 00-8 8v2m18-10l-6-6m6 6l-6 6"/></svg>'

      def initialize(conversation_thread:, person_first_name:, newest_thread: false, can_send: false, scout_draft: nil)
        @ct = conversation_thread
        @person_first_name = person_first_name
        @newest_thread = newest_thread
        @can_send = can_send
        @scout_draft = scout_draft
      end

      def view_template
        div(class: "mb-4 overflow-hidden rounded-xl border border-border bg-card") do
          heading_row
          messages_list
          reply_area if @newest_thread && @can_send
        end
      end

      private

      def heading_row
        div(class: "flex min-w-0 items-start gap-2 border-b border-border px-4 py-3") do
          div(class: "min-w-0 flex-1") do
            h3(class: "text-[15px] font-semibold tracking-[-0.01em] text-foreground") do
              plain(@ct.subject.presence || t(".no_subject"))
            end
            if @ct.latest_at
              span(class: "text-[12px] text-muted-foreground") do
                plain("#{t('.messages', count: @ct.count)} · #{thread_date(@ct.latest_at)}")
              end
            end
          end
          if @ct.newest
            a(href: helpers.email_message_path(@ct.newest),
              data: { turbo_frame: "_top" },
              class: "flex-shrink-0 self-center text-[12px] text-muted-foreground no-underline underline-offset-2 hover:text-foreground hover:underline") do
              plain(t(".open_in_inbox"))
            end
          end
        end
      end

      def messages_list
        div(class: "divide-y divide-border/50") do
          @ct.older.each do |message|
            render(ThreadMessage.new(message: message, person_first_name: @person_first_name, open: false))
          end
          if @ct.newest
            render(ThreadMessage.new(message: @ct.newest, person_first_name: @person_first_name, open: true,
                                     full_body: @newest_thread))
          end
        end
      end

      def reply_area
        div(class: "border-t border-border px-4 py-3") do
          if @scout_draft.present?
            draft_card
          else
            ghost_row
          end
        end
      end

      def draft_card
        div(class: "scout-glass rounded-xl px-4 pb-2 pt-3") do
          stripped = helpers.strip_tags(@scout_draft).squish.truncate(400)
          p(class: "whitespace-pre-line text-[13px] leading-relaxed text-foreground/90") { plain(stripped) }
          div(class: "mt-2 flex items-center gap-2") do
            span(class: "inline-flex items-center gap-1.5 rounded-md border border-[color:var(--scout-border)] px-2 py-1 text-[11.5px] font-medium text-foreground") do
              raw(safe(SPARK_ICON))
              plain(t(".draft_by_scout"))
            end
            span(class: "flex-1")
            if (target = @ct.reply_target)
              open_draft_form(target)
            end
          end
        end
      end

      def open_draft_form(target)
        form(action: helpers.compose_email_message_path(target, mode: :reply),
             method: "post", class: "inline-flex") do
          input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
          button(type: "submit",
                 class: "inline-flex cursor-pointer items-center gap-1.5 rounded-lg bg-primary px-3 py-1.5 text-[13px] font-semibold text-primary-foreground transition-colors hover:bg-primary/90") do
            plain(t(".open_draft"))
          end
        end
      end

      def ghost_row
        div(class: "flex items-center gap-2") do
          if (target = @ct.reply_target)
            compose_form_button(target, :reply, REPLY_ICON, t(".reply"))
            compose_form_button(target, :reply_all, REPLY_ALL_ICON, t(".reply_all"))
          end
          if @ct.newest
            compose_form_button(@ct.newest, :forward, FORWARD_ICON, t(".forward"))
          end
        end
      end

      def compose_form_button(message, mode, icon_svg, label)
        form(action: helpers.compose_email_message_path(message, mode: mode),
             method: "post", class: "inline-flex") do
          input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
          button(type: "submit",
                 class: "inline-flex cursor-pointer items-center gap-1.5 rounded-lg border border-border px-3 py-1.5 text-[12px] font-medium text-foreground hover:bg-secondary") do
            raw(safe(icon_svg))
            whitespace
            plain(label)
          end
        end
      end

      # Today → clock; this year → day_month; older → full date.
      def thread_date(time)
        if time.to_date == Date.current
          l(time, format: :clock)
        elsif time.year == Date.current.year
          l(time.to_date, format: :day_month)
        else
          l(time.to_date, format: :date)
        end
      end
    end
  end
end
