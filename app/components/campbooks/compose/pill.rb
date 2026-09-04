# frozen_string_literal: true

module Campbooks
  module Compose
    # The parked-draft affordance: a small frosted capsule (bottom-right, above
    # the mobile bottom nav) that survives navigation and reopens the draft in
    # the Dock on click. It rests quiet — a muted glyph — and reveals the draft's
    # destination on hover/focus; the × sets it aside (server-side dismissed_at +
    # Undo toast) without deleting the draft, and editing the draft again revives
    # it. Shares the frosted card/border/blur language of the sync pill and action
    # toasts, and never uses Ember: a saved draft isn't Scout, live, or a win.
    class Pill < Campbooks::Base
      def initialize(draft:)
        @draft = draft
      end

      def view_template
        div(id: "compose_draft_pill",
            class: "group fixed bottom-20 right-4 z-[60] inline-flex items-center gap-1 rounded-full " \
                   "border border-border bg-card/95 py-1.5 pl-1.5 pr-2 shadow-lg backdrop-blur " \
                   "transition-shadow hover:shadow-xl lg:bottom-5",
            data: { compose_dock_target: "pill" }) do
          resume_link
          dismiss_button
        end
      end

      private

      def resume_link
        a(href: helpers.draft_email_path(@draft),
          class: "flex min-w-0 items-center gap-2 rounded-full focus:outline-none " \
                 "focus-visible:ring-2 focus-visible:ring-accent-400",
          data: { turbo_stream: "true" },
          aria_label: t(".resume_aria")) do
          glyph
          detail
        end
      end

      # Quiet pencil in a muted circle — the resting state. Muted, not Ember.
      def glyph
        span(class: "flex h-7 w-7 flex-shrink-0 items-center justify-center rounded-full " \
                    "bg-muted text-muted-foreground") do
          svg(class: "h-3.5 w-3.5", fill: "none", stroke: "currentColor", stroke_width: "2.2",
              stroke_linecap: "round", stroke_linejoin: "round", viewBox: "0 0 24 24") do
            raw(safe('<path d="M12 20h9"/><path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4Z"/>'))
          end
        end
      end

      # The draft's destination, collapsed to zero width at rest and revealed on
      # hover/focus (grid 0fr→1fr + fade). overflow-hidden clips it while closed.
      def detail
        span(class: "grid grid-cols-[0fr] opacity-0 transition-all duration-200 ease-out " \
                    "group-hover:grid-cols-[1fr] group-hover:opacity-100 " \
                    "group-focus-within:grid-cols-[1fr] group-focus-within:opacity-100 " \
                    "motion-reduce:transition-none") do
          span(class: "overflow-hidden") do
            span(class: "block max-w-[168px] truncate whitespace-nowrap pl-0.5 pr-1 " \
                        "text-xs font-medium text-foreground") { title_text }
          end
        end
      end

      # Mirrors ActionToast#undo_button: a tiny inline form so the x can POST (with
      # CSRF) as a Turbo Stream from any page the pill renders on. Stays reachable
      # at rest (no hover) so it works on touch.
      def dismiss_button
        form(action: helpers.dismiss_draft_email_path(@draft), method: :post, class: "contents") do
          input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
          button(type: "submit",
                 class: "flex h-6 w-6 flex-shrink-0 items-center justify-center rounded-full " \
                        "text-muted-foreground transition-colors hover:bg-muted hover:text-foreground " \
                        "focus:outline-none focus-visible:ring-2 focus-visible:ring-accent-400",
                 title: t(".dismiss_title"), aria_label: t(".dismiss_title")) do
            svg(class: "h-3 w-3", fill: "none", stroke: "currentColor", stroke_width: "2.4",
                stroke_linecap: "round", viewBox: "0 0 24 24") do
              raw(safe('<path d="M18 6 6 18"/><path d="m6 6 12 12"/>'))
            end
          end
        end
      end

      def title_text
        @draft.display_title.presence || t(".untitled")
      end
    end
  end
end
