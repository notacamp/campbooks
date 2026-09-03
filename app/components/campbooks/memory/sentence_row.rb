# frozen_string_literal: true

module Campbooks
  module Memory
    # One line of Scout's memory: the editable sentence (with bold spans) on the
    # left; on the right its origin label (muted "Taught by you · Jul 12", or an
    # Ember spark + "Learned from …", or "Default") and its actions (edit → the
    # settings form, remove → trash, Confirm → a ghost button for learned habits).
    # Mobile: origin + actions wrap under the sentence.
    class SentenceRow < Campbooks::Base
      SPARK = '<svg viewBox="0 0 24 24" fill="currentColor" class="h-3 w-3" aria-hidden="true">' \
              '<path d="M12 3l1.4 5.1L18.5 9l-5.1 1.4L12 15l-1.4-4.6L5.5 9l5.1-.9z"/></svg>'
      PENCIL = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" ' \
               'stroke-linecap="round" stroke-linejoin="round" class="h-4 w-4" aria-hidden="true">' \
               '<path d="M12 20h9"/><path d="M16.5 3.5a2.12 2.12 0 0 1 3 3L7 19l-4 1 1-4z"/></svg>'
      TRASH = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" ' \
              'stroke-linecap="round" stroke-linejoin="round" class="h-4 w-4" aria-hidden="true">' \
              '<path d="M3 6h18M8 6V4a1 1 0 0 1 1-1h6a1 1 0 0 1 1 1v2m2 0-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6"/></svg>'

      def self.dom_id(entry)
        "mem-#{entry.id.gsub(/[^a-zA-Z0-9_-]/, '-')}"
      end

      def initialize(entry:)
        @entry = entry
      end

      def view_template
        div(
          id: self.class.dom_id(@entry),
          class: "flex flex-col gap-1.5 py-3 text-[14px] sm:grid sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center sm:gap-4",
          data: { memory_target: "row", memory_text: @entry.plain.downcase }
        ) do
          sentence
          div(class: "flex items-center gap-3 sm:justify-end") do
            origin
            actions
          end
        end
      end

      private

      def sentence
        span(class: "leading-snug text-foreground") do
          @entry.sentence.spans.each do |span|
            if span[:bold]
              b(class: "font-semibold") { span[:text] }
            else
              plain(span[:text])
            end
          end
        end
      end

      def origin
        classes = class_names("flex items-center gap-1 whitespace-nowrap text-[11.5px]",
          @entry.learned? ? "font-medium" : "text-muted-foreground")
        div(class: classes, style: (@entry.learned? ? "color: var(--ember-solid)" : nil)) do
          raw(safe(SPARK)) if @entry.learned?
          span { @entry.origin_detail }
        end
      end

      def actions
        return if @entry.actions.empty?

        div(class: "flex flex-shrink-0 items-center gap-1") do
          confirm_button if @entry.confirmable?
          edit_link if @entry.editable? && @entry.form_path
          remove_button if @entry.removable?
        end
      end

      def edit_link
        a(
          href: @entry.form_path,
          class: "flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-secondary hover:text-foreground",
          aria_label: t(".edit")
        ) { raw(safe(PENCIL)) }
      end

      def confirm_button
        action_form(helpers.settings_memory_entry_confirm_path(@entry.id)) do
          button(
            type: "submit",
            class: "rounded-md border border-border px-2.5 py-1 text-[12.5px] font-medium text-foreground transition-colors hover:bg-secondary"
          ) { t(".confirm") }
        end
      end

      def remove_button
        action_form(helpers.settings_memory_entry_path(@entry.id), method: "delete",
          confirm: @entry.learned? ? t(".remove_habit_confirm") : t(".remove_confirm")) do
          button(
            type: "submit",
            class: "flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-destructive/10 hover:text-destructive",
            aria_label: t(".remove")
          ) { raw(safe(TRASH)) }
        end
      end

      # A minimal inline form (class="contents" so it doesn't disturb the flex row)
      # carrying CSRF + an optional method override and turbo-confirm.
      def action_form(url, method: "post", confirm: nil, &block)
        form(action: url, method: "post", class: "contents",
          data: (confirm ? { turbo_confirm: confirm } : {})) do
          input(type: "hidden", name: "_method", value: method) unless method == "post"
          input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
          yield
        end
      end
    end
  end
end
