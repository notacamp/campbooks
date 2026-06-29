# frozen_string_literal: true

module Campbooks
  module Files
    # "Manage access" panel for a folder (Files Phase 3). A restrict toggle (open =
    # whole workspace, restricted = members + admins only); when restricted, a member
    # list with role selects + remove, and an add-person form. Mirrors
    # Campbooks::EmailAccountSharing; every control PATCHes /files/folders/:id/share.
    class FolderSharing < Campbooks::Base
      SELECT = "rounded-md border-gray-300 shadow-sm text-xs text-foreground focus:border-accent-500 focus:ring-accent-500 cursor-pointer"

      def initialize(folder:, members:, addable_users:, current_user:)
        @folder = folder
        @members = members
        @addable_users = addable_users
        @current_user = current_user
      end

      def view_template
        div(class: "space-y-5") do
          restrict_toggle
          if @folder.restricted?
            members_list
            add_person_section
          else
            p(class: "text-xs text-muted-foreground") { t(".open_hint") }
          end
        end
      end

      private

      def restrict_toggle
        form(action: helpers.files_folder_share_path(@folder), method: "post", data: { controller: "auto-submit" }) do
          patch_and_token
          label(class: "flex cursor-pointer items-start gap-3 rounded-lg border border-border p-3") do
            input(type: "hidden", name: "restricted", value: "false")
            input(type: "checkbox", name: "restricted", value: "true", checked: @folder.restricted?,
              class: "mt-0.5 rounded border-gray-300 text-accent-600 focus:ring-accent-500",
              data: { action: "change->auto-submit#submit" })
            span do
              span(class: "block text-[13px] font-medium text-foreground") { t(".restrict_label") }
              span(class: "block text-xs text-muted-foreground") { t(".restrict_hint") }
            end
          end
        end
      end

      def members_list
        div(class: "space-y-2") { @members.each { |m| member_row(m) } }
      end

      def member_row(member)
        div(class: "flex items-center gap-3 rounded-lg border border-border p-3") do
          render Campbooks::ContactAvatar.new(email: member.user.email_address, size: :md, variant: :accent)
          div(class: "min-w-0 flex-1") do
            div(class: "flex items-center gap-1.5") do
              span(class: "truncate text-[13px] font-medium text-foreground") { member.user.name.presence || member.user.email_address }
              span(class: "flex-shrink-0 text-xs text-muted-foreground") { t(".you_badge") } if member.user_id == @current_user.id
            end
            p(class: "truncate text-xs text-muted-foreground") { member.user.email_address }
          end
          if member.owner?
            render(Campbooks::Badge.new(variant: :accent, size: :md)) { t(".owner_badge") }
          else
            div(class: "flex flex-shrink-0 items-center gap-1.5") do
              role_select(member)
              remove_form(member)
            end
          end
        end
      end

      def role_select(member)
        form(action: helpers.files_folder_share_path(@folder), method: "post", data: { controller: "auto-submit" }) do
          patch_and_token
          input(type: "hidden", name: "user_email", value: member.user.email_address)
          select(name: "role", "aria-label": t(".role_aria"), class: SELECT, data: { action: "change->auto-submit#submit" }) do
            MailFolderUser::ROLES.each do |role|
              option(value: role, selected: member.role == role) { t(".role_#{role}") }
            end
          end
        end
      end

      def remove_form(member)
        form(action: helpers.files_folder_share_path(@folder), method: "post",
          data: { turbo_confirm: t(".remove_confirm", name: member.user.name.presence || member.user.email_address) }) do
          patch_and_token
          input(type: "hidden", name: "user_email", value: member.user.email_address)
          input(type: "hidden", name: "remove", value: "true")
          button(type: "submit", aria_label: t(".remove_aria"),
            class: "cursor-pointer rounded-md p-1.5 text-muted-foreground hover:bg-red-50 hover:text-red-600 dark:hover:bg-red-500/10") do
            raw(safe('<svg class="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.8"><path stroke-linecap="round" stroke-linejoin="round" d="M19 7l-.8 12.1a2 2 0 01-2 1.9H7.8a2 2 0 01-2-1.9L5 7m5 4v6m4-6v6M9 7V4a1 1 0 011-1h4a1 1 0 011 1v3m-9 0h14"/></svg>'))
          end
        end
      end

      def add_person_section
        div(class: "rounded-lg border border-border bg-muted/30 p-4") do
          h4(class: "mb-3 text-xs font-semibold text-foreground") { t(".add_title") }
          if @addable_users.empty?
            p(class: "text-xs text-muted-foreground") { t(".all_have_access") }
          else
            add_person_form
          end
        end
      end

      def add_person_form
        form(action: helpers.files_folder_share_path(@folder), method: "post", class: "flex flex-col gap-2 sm:flex-row") do
          patch_and_token
          select(name: "user_email", required: true, class: "#{SELECT} flex-1") do
            option(value: "") { t(".select_placeholder") }
            @addable_users.each { |u| option(value: u.email_address) { "#{u.name} (#{u.email_address})" } }
          end
          select(name: "role", "aria-label": t(".role_aria"), class: "#{SELECT} sm:w-32") do
            MailFolderUser::ROLES.each { |role| option(value: role, selected: role == "viewer") { t(".role_#{role}") } }
          end
          button(type: "submit",
            class: "inline-flex cursor-pointer items-center justify-center rounded-lg bg-accent-600 px-4 py-1.5 text-xs font-medium text-white hover:bg-accent-700") do
            plain t(".add_button")
          end
        end
      end

      def patch_and_token
        input(type: "hidden", name: "_method", value: "patch")
        input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
      end
    end
  end
end
