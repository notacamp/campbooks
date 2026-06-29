# frozen_string_literal: true

module Campbooks
  # "Use template" control for the composer: a small trigger button plus a hidden
  # modal whose body is a lazy turbo-frame. Opening it loads the template list
  # (GET /email_templates); picking one loads its variables form into the same
  # frame; inserting renders the subject/body and attaches the document-template
  # PDFs into the surrounding compose <form> (email-template-picker controller).
  #
  # Rendered inside the compose form so the Stimulus controller can reach the
  # subject input, the TipTap body editor, and the attachments tray via
  # `this.element.closest("form")`.
  class EmailTemplatePicker < Campbooks::Base
    # frame_id is made unique per compose surface so two open composers (drawer +
    # thread) don't collide on one turbo-frame id. The server echoes it back via
    # the Turbo-Frame request header, so the list/fill fragments target the right
    # frame automatically.
    def initialize(frame_id: "email_template_picker_frame", open: false)
      @frame_id = frame_id
      @open = open
    end

    def view_template
      div(class: "inline-flex",
          data: {
            controller: "email-template-picker",
            email_template_picker_list_url_value: helpers.email_templates_path
          }) do
        trigger_button
        modal
      end
    end

    private

    def trigger_button
      button(type: "button",
             class: "inline-flex items-center gap-1 px-3 py-1.5 text-xs font-medium text-gray-600 hover:text-gray-900 border border-gray-200 rounded-lg hover:bg-gray-50 transition-colors cursor-pointer",
             data: { action: "email-template-picker#open" },
             aria_label: t(".button_aria")) do
        svg(class: "w-3.5 h-3.5", fill: "none", stroke: "currentColor", viewBox: "0 0 24 24") do
          raw(safe('<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"/>'))
        end
        plain t(".button")
      end
    end

    def modal
      div(class: class_names(("hidden" unless @open), "fixed inset-0 z-50 flex items-start justify-center p-4 pt-[10vh] bg-black/50 backdrop-blur-sm"),
          data: { email_template_picker_target: "modal", action: "click->email-template-picker#backdropClose keydown->email-template-picker#keydown" },
          role: "dialog", aria_modal: "true", aria_label: t(".title"), tabindex: "-1") do
        div(class: "bg-card text-card-foreground rounded-xl shadow-xl border border-gray-200 w-full max-w-lg max-h-[80vh] flex flex-col overflow-hidden text-left") do
          header
          div(class: "flex-1 overflow-y-auto p-2") { frame }
        end
      end
    end

    def header
      div(class: "flex items-start justify-between gap-2 px-4 py-3 border-b border-gray-200") do
        div do
          h2(class: "text-sm font-semibold text-gray-900") { t(".title") }
          p(class: "text-xs text-gray-500") { t(".subtitle") }
        end
        button(type: "button", class: "text-gray-400 hover:text-gray-600 flex-shrink-0 cursor-pointer",
               aria_label: t(".close"), data: { action: "email-template-picker#close" }) do
          svg(class: "w-4 h-4", fill: "none", stroke: "currentColor", viewBox: "0 0 24 24") do
            raw(safe('<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/>'))
          end
        end
      end
    end

    def frame
      raw(helpers.turbo_frame_tag(@frame_id, class: "block",
            data: { email_template_picker_target: "frame" }) do
        helpers.content_tag(:div, t("shared.actions.loading"),
                            class: "flex items-center justify-center py-16 text-sm text-gray-400")
      end)
    end
  end
end
