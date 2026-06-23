# frozen_string_literal: true

module Campbooks
  class ContactPopover < Campbooks::Base
    def initialize(contact:, **attrs)
      @contact = contact
      @person = contact.person
      @attrs = attrs
    end

    def view_template
      div(class: "bg-card rounded-xl shadow-lg border border-gray-200 p-4 w-72", **@attrs) do
        # Header: avatar + name
        div(class: "flex items-center gap-2.5 mb-3") do
          render(ContactAvatar.new(
            email: @person&.name.presence || @contact.email,
            size: :lg,
            variant: :accent
          ))
          div(class: "min-w-0") do
            div(class: "text-sm font-semibold text-gray-900 truncate") do
              plain(@person&.name.presence || @contact.display_name)
            end
            div(class: "text-[11px] text-gray-400 truncate") { plain(@contact.email) }
          end
        end

        # Stats row
        div(class: "flex items-center gap-4 mb-3") do
          div(class: "flex items-center gap-1") do
            span(class: "text-xs font-semibold text-gray-700") { plain(@contact.email_count.to_s) }
            span(class: "text-[10px] text-gray-400") { t(".email_count_label") }
          end
          if @contact.last_email_at
            div(class: "flex items-center gap-1") do
              span(class: "text-[10px] text-gray-400") { t(".last_email_label") }
              span(class: "text-xs text-gray-600") { plain(l(@contact.last_email_at, format: :full)) }
            end
          end
        end

        # Relationship
        if @contact.relationship_type.present?
          div(class: "mb-2") do
            render_relationship_badge
          end
        end

        # AI summary
        if @contact.context_summary.present?
          div(class: "mb-3") do
            p(class: "text-xs text-gray-500 leading-relaxed") { plain(@contact.context_summary) }
          end
        end

        # Actions: star / block (re-rendered in place by set_state.turbo_stream)
        div(class: "pt-3 border-t border-gray-100 dark:border-gray-700 space-y-2.5") do
          render(ContactStateActions.new(contact: @contact))

          a(
            href: helpers.contact_path(@contact),
            class: "inline-flex items-center gap-1 text-xs font-medium text-accent-600 hover:text-accent-700",
            data: { turbo_frame: "_top" }
          ) do
            plain(t(".view_profile"))
          end
        end
      end
    end

    private

    def render_relationship_badge
      colors = {
        "client" => "bg-green-100 text-green-700 dark:bg-green-500/15 dark:text-green-300",
        "vendor" => "bg-blue-100 text-blue-700 dark:bg-blue-500/15 dark:text-blue-300",
        "partner" => "bg-purple-100 text-purple-700 dark:bg-purple-500/15 dark:text-purple-300",
        "service_provider" => "bg-orange-100 text-orange-700 dark:bg-orange-500/15 dark:text-orange-300",
        "colleague" => "bg-gray-100 text-gray-700",
        "personal" => "bg-pink-100 text-pink-700 dark:bg-pink-500/15 dark:text-pink-300",
        "unknown" => "bg-gray-100 text-gray-500"
      }
      css = colors[@contact.relationship_type] || "bg-gray-100 text-gray-500"

      span(class: "inline-flex items-center rounded-full px-2 py-0.5 text-[10px] font-medium #{css}") do
        plain(helpers.human_enum(Person, :relationship_type, @contact.relationship_type))
      end
    end
  end
end
