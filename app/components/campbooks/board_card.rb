# frozen_string_literal: true

module Campbooks
  # A draggable thread card on the inbox status board. Shows the latest message's
  # avatar, subject, sender and date. It is an <a> to the message, so a plain
  # click opens the bottom-right email drawer (email-drawer intercepts it in the
  # Board layout) while dragging is handled by the inbox-board controller.
  class BoardCard < Campbooks::Base
    def initialize(thread:, column_key:, draggable: true)
      @thread = thread
      @column_key = column_key
      @draggable = draggable
    end

    def view_template
      message = @thread.latest_message
      return unless message

      participants = @thread.participant_senders(limit: 4)
      multi = participants.size > 1

      a(
        href: helpers.email_message_path(message),
        class: class_names(
          "group block rounded-lg border border-border bg-card p-2.5 shadow-sm transition hover:border-accent-300 hover:shadow-md",
          @draggable ? "cursor-grab active:cursor-grabbing" : "cursor-pointer"
        ),
        draggable: @draggable.to_s,
        data: {
          inbox_board_target: "card",
          board_card: "",
          thread_id: @thread.id,
          column: @column_key
        }
      ) do
        div(class: "flex items-start gap-2") do
          if multi
            render(Campbooks::ContactAvatarGroup.new(
              participants: participants, size: :sm, max: 3, variant: :neutral,
              account_color: @thread.email_account&.color
            ))
          else
            render(Campbooks::ContactAvatar.new(
              email: message.from_address || "?",
              sent: message.sent?,
              size: :sm,
              contact_id: message.contact_id,
              variant: :neutral,
              show_direction: false,
              account_color: @thread.email_account&.color
            ))
          end
          div(class: "min-w-0 flex-1") do
            div(class: "flex items-start justify-between gap-1.5") do
              span(class: "truncate text-[12px] font-semibold leading-snug text-foreground") { message.subject.presence || t(".no_subject") }
              span(class: "flex-shrink-0 text-[10px] text-muted-foreground") { helpers.thread_date_label(message.received_at) }
            end
            span(class: "mt-0.5 block truncate text-[11px] text-muted-foreground") { sender_label(message) }
            snooze_hint if @column_key == :snoozed && @thread.snoozed_until.present?
          end
        end
      end
    end

    private

    def sender_label(message)
      message.from_address.to_s.split("@").first.presence || "—"
    end

    def snooze_hint
      span(class: "mt-1 inline-flex items-center gap-1 text-[10px] font-medium text-amber-600 dark:text-amber-300") do
        plain("#{t('.snoozed_prefix')} #{l(@thread.snoozed_until, format: :at)}")
      end
    end
  end
end
