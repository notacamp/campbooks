# frozen_string_literal: true

module Campbooks
  module Files
    # A filed email as a list item in the Files area (shown inside a folder it's been
    # filed into). Links to the message in Mail; the kebab opens it or removes it
    # from the folder. Non-Turbo membership form (redirects back). The email itself
    # is never deleted from here — only its folder membership.
    class EmailRow < Campbooks::Base
      def initialize(email:, current_folder: nil)
        @email = email
        @current_folder = current_folder
      end

      def view_template
        div(id: helpers.dom_id(@email, :files),
          class: "flex items-center gap-3 rounded-xl border border-gray-200 bg-card px-4 py-3 shadow-sm dark:border-white/10") do
          a(href: helpers.email_message_path(@email), class: "flex min-w-0 flex-1 items-center gap-3") do
            span(class: "flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-lg bg-muted text-muted-foreground") { raw(safe(MAIL_ICON)) }
            span(class: "min-w-0 flex-1") do
              span(class: "block truncate text-sm font-medium text-foreground") { subject }
              span(class: "mt-0.5 block truncate text-xs text-muted-foreground") { meta }
            end
          end
          menu
        end
      end

      private

      def subject
        @email.subject.presence || t(".no_subject")
      end

      def meta
        parts = [ @email.from_address.presence ]
        parts << l(@email.received_at.to_date, format: :long) if @email.received_at
        parts.compact.join(" · ")
      end

      def menu
        details(class: "relative inline-block flex-shrink-0 text-left", data: { controller: "dropdown-close" }) do
          summary(class: "inline-flex h-8 w-8 cursor-pointer list-none items-center justify-center rounded-md text-muted-foreground hover:bg-muted [&::-webkit-details-marker]:hidden",
            aria: { label: t(".actions") }) { raw(safe(DOTS_ICON)) }
          div(class: "absolute right-0 z-20 mt-1 w-52 rounded-lg border border-border bg-card p-1 text-left shadow-lg") do
            a(href: helpers.email_message_path(@email), class: menu_item) { t(".open") }
            remove_item if @current_folder
          end
        end
      end

      def remove_item
        membership = @current_folder.folder_memberships.find_by(folderable: @email)
        return unless membership

        form(action: helpers.folder_membership_path(membership), method: "post", class: "block", data: { turbo: false }) do
          input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
          input(type: "hidden", name: "_method", value: "delete")
          button(type: "submit", class: menu_item) { t(".remove_from_folder") }
        end
      end

      def menu_item
        "block w-full cursor-pointer rounded-md px-2 py-1.5 text-left text-sm text-foreground hover:bg-muted"
      end

      MAIL_ICON = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" class="h-[18px] w-[18px]"><rect x="3" y="5" width="18" height="14" rx="2"/><path d="m3 7 9 6 9-6"/></svg>'
      DOTS_ICON = '<svg viewBox="0 0 24 24" fill="currentColor" class="h-4 w-4"><circle cx="12" cy="5" r="1.6"/><circle cx="12" cy="12" r="1.6"/><circle cx="12" cy="19" r="1.6"/></svg>'
    end
  end
end
