# frozen_string_literal: true

module Campbooks
  class EmailListItem < Campbooks::Base
    def initialize(message:, **attrs)
      @message = message
      @attrs = attrs
    end

    def view_template
      div(class: "px-5 py-3 hover:bg-gray-50 transition-colors flex items-center gap-3", **@attrs) do
        # Avatar + direction indicator
        render_avatar

        # Content
        div(class: "flex-1 min-w-0") do
          div(class: "flex items-center gap-2") do
            a(
              href: helpers.email_message_path(@message),
              class: "text-sm font-medium text-accent-600 hover:text-accent-700 truncate"
            ) do
              @message.subject.presence || t(".no_subject")
            end
          end
          div(class: "flex items-center gap-2 mt-0.5") do
            span(class: "text-[11px] text-gray-400") { sender_label }
            if @message.ai_summary.present?
              span(class: "text-xs text-gray-500 truncate") { @message.ai_summary.truncate(80) }
            end
          end
        end

        # Right side: tags + date
        div(class: "flex items-center gap-1.5 flex-shrink-0") do
          tags = Tag.visible_for(Current.workspace).where(id: @message.tags.select(:id))
          tags.first(2).each do |tag|
            span(
              class: "inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-medium",
              style: "background-color: #{tag.color}20; color: #{tag.color}"
            ) { tag.name }
          end
          span(class: "text-[10px] text-gray-400 flex-shrink-0 ml-1") do
            @message.received_at ? l(@message.received_at, format: :full) : nil
          end
        end
      end
    end

    private

    def render_avatar
      is_sent = @message.sent?
      email = is_sent ? @message.to_address.presence : @message.from_address

      render(Campbooks::ContactAvatar.new(
        email: email || "?",
        sent: is_sent,
        size: :lg,
        contact_id: @message.contact_id,
        variant: :neutral,
        show_direction: true,
        account_color: @message.email_account&.color
      ))
    end

    def sender_label
      is_sent = @message.sent?
      if is_sent
        t(".sent_to", address: clean_address(@message.to_address))
      else
        clean_address(@message.from_address) || @message.from_address
      end
    end

    def clean_address(raw)
      return nil unless raw
      raw.split("<").first&.strip&.presence || raw
    end
  end
end
