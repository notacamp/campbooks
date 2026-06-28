# frozen_string_literal: true

module Campbooks
  module Pipeline
    # One item (document or email) on a pipeline board. The whole card is the
    # draggable unit (so its remove button travels with it between columns); the
    # body is a link to the underlying record, and a hover-revealed button removes
    # it from the pipeline.
    class BoardCard < Campbooks::Base
      def initialize(membership:, pipeline:, draggable: true)
        @membership = membership
        @item = membership.item
        @pipeline = pipeline
        @draggable = draggable
      end

      def view_template
        return unless @item

        div(
          class: class_names(
            "group/card relative rounded-lg border border-border bg-card shadow-sm transition hover:border-accent-300 hover:shadow-md",
            @draggable ? "cursor-grab active:cursor-grabbing" : ""
          ),
          draggable: @draggable.to_s,
          data: {
            action: "dragstart->pipeline-board#dragStart dragend->pipeline-board#dragEnd",
            membership_id: @membership.id,
            stage_id: @membership.current_stage_id
          }
        ) do
          a(href: item_path, class: "block p-2.5 pr-7") do
            div(class: "flex items-start justify-between gap-1.5") do
              span(class: "truncate text-[12px] font-semibold leading-snug text-foreground") { title }
              span(class: "flex-shrink-0 text-[10px] text-muted-foreground") { helpers.time_ago_in_words(timestamp) }
            end
            span(class: "mt-0.5 block truncate text-[11px] text-muted-foreground") { subtitle }
          end
          remove_button
        end
      end

      private

      def remove_button
        form(
          action: helpers.pipeline_membership_path(@pipeline, @membership),
          method: "post",
          class: "absolute right-1 top-1 opacity-0 transition group-hover/card:opacity-100",
          data: { turbo_confirm: t(".remove_confirm") }
        ) do
          input(type: "hidden", name: "_method", value: "delete")
          input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
          button(
            type: "submit",
            class: "flex h-5 w-5 items-center justify-center rounded text-muted-foreground/50 hover:bg-muted hover:text-red-500",
            aria_label: t(".remove")
          ) do
            svg(class: "h-3.5 w-3.5", fill: "none", stroke: "currentColor", stroke_width: "2", viewBox: "0 0 24 24", aria_hidden: "true") do
              raw(safe('<path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12"/>'))
            end
          end
        end
      end

      def document? = @item.is_a?(Document)

      def item_path
        document? ? helpers.document_path(@item) : helpers.email_message_path(@item)
      end

      def title
        if document?
          @item.display_title
        else
          @item.subject.presence || t(".no_subject")
        end
      end

      def subtitle
        if document?
          @item.classification&.name.to_s
        else
          @item.from_address.to_s.split("@").first.to_s
        end
      end

      def timestamp
        document? ? @item.created_at : @item.received_at
      end
    end
  end
end
