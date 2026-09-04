# frozen_string_literal: true

module Campbooks
  module People
    # One email thread in the Person conversation pane. Renders as:
    #   * a heading row (subject, message count + date)
    #   * the newest message open first (ThreadMessage, open: true), with its own
    #     Reply / Reply all / Forward row when can_send is true
    #   * when this is the newest thread and a Scout draft exists: the draft card
    #     directly below the newest message, inside the same divide-y list
    #   * older messages folded beneath the newest, newest-first (reverse_each)
    #
    # Only the newest thread arrives with its messages loaded. Every other thread
    # renders its heading over a lazy turbo-frame that People::ThreadsController
    # fills in when the card scrolls into view; inside a loaded thread the folded
    # (older) messages fetch their bodies the same way, through
    # People::MessagesController, when expanded. Both need `person_id`.
    #
    # @param conversation_thread [People::ConversationThread]
    # @param person_first_name [String]
    # @param person_id [String, nil] the person whose conversation this is (lazy URLs)
    # @param newest_thread [Boolean] true for the first thread on page 1
    # @param can_send [Boolean]
    # @param scout_draft [String, nil] pre-fetched draft content (or nil)
    # @param frame_only [Boolean] render just the message list (the lazy frame's content)
    class ThreadBlock < Campbooks::Base
      register_element :turbo_frame

      CLIP_ICON = '<svg class="h-3.5 w-3.5 flex-shrink-0 text-muted-foreground" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15.172 7l-6.586 6.586a2 2 0 102.828 2.828l6.414-6.586a4 4 0 00-5.656-5.656l-6.415 6.585a6 6 0 108.486 8.486L20.5 13"/></svg>'
      SPARK_ICON = '<svg viewBox="0 0 24 24" fill="currentColor" class="h-3 w-3 flex-shrink-0" style="color:var(--ember-solid)" aria-hidden="true"><path d="M12 5l1.7 5.6L19.5 12l-5.8 1.4L12 19l-1.7-5.6L4.5 12l5.8-1.4z"/></svg>'

      def initialize(conversation_thread:, person_first_name:, person_id: nil, newest_thread: false,
                     can_send: false, scout_draft: nil, frame_only: false)
        @ct = conversation_thread
        @person_first_name = person_first_name
        @person_id = person_id
        @newest_thread = newest_thread
        @can_send = can_send
        @scout_draft = scout_draft
        @frame_only = frame_only
      end

      def view_template
        return messages_list if @frame_only

        div(class: "mb-4 overflow-hidden rounded-xl border border-border bg-card") do
          heading_row
          if @ct.loaded? || @person_id.nil?
            messages_list
          else
            lazy_messages
          end
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
        end
      end

      # The heading's lazy frame: People::ThreadsController#show replaces it with
      # the message list once the card scrolls into view.
      def lazy_messages
        turbo_frame(id: "people_thread_#{@ct.thread.id}",
                    src: helpers.people_thread_path(@person_id, @ct.thread.id),
                    loading: "lazy", class: "block") do
          div(class: "flex items-center justify-center py-4") { render(Campbooks::Spinner.new(size: :sm)) }
        end
      end

      def messages_list
        div(class: "divide-y divide-border/50") do
          if @ct.newest
            render(ThreadMessage.new(message: @ct.newest, person_first_name: @person_first_name, open: true,
                                     full_body: @newest_thread, can_send: @can_send))
          end
          if @newest_thread && @can_send && @scout_draft.present?
            div(class: "px-4 py-3") { draft_card }
          end
          @ct.older.reverse_each do |message|
            render(ThreadMessage.new(message: message, person_first_name: @person_first_name, open: false,
                                     lazy_src: lazy_message_src(message), can_send: @can_send))
          end
        end
      end

      # Folded messages fetch their body on expand (nil = render it inline, as the
      # previews and the frame-less callers do).
      def lazy_message_src(message)
        @person_id ? helpers.people_message_path(@person_id, message.id) : nil
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
