# frozen_string_literal: true

module Campbooks
  # Owner-facing "Manage access" panel for a shared email account: lists everyone
  # with access (owner first), lets the owner change each collaborator's role or
  # remove them, and add new workspace members. Every control posts to the
  # existing PATCH /email_accounts/:id endpoint (update_user_permissions).
  class EmailAccountSharing < Campbooks::Base
    SELECT_CLASSES = "rounded-md border-gray-300 shadow-sm text-xs text-foreground focus:border-accent-500 focus:ring-accent-500 cursor-pointer"

    # @param account [EmailAccount]
    # @param members [Array<EmailAccountUser>] ordered owner-first
    # @param addable_users [Array<User>] workspace users not yet on the account
    # @param current_user [User] the viewer, to mark their own row
    def initialize(account:, members:, addable_users:, current_user:)
      @account = account
      @members = members
      @addable_users = addable_users
      @current_user = current_user
    end

    def view_template
      div(data: { section: "account_sharing" }) do
        panel_header
        div(class: "px-6 py-5 space-y-6") do
          members_list
          add_person_section
        end
      end
    end

    private

    def panel_header
      header(class: "sticky top-0 z-10 bg-background/95 backdrop-blur px-6 py-4 border-b border-border") do
        a(href: helpers.inbox_settings_accounts_path,
          data: { turbo_frame: "inbox_settings_panel" },
          class: "inline-flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground mb-2 transition-colors") do
          raw(safe('<svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M15 19l-7-7 7-7"/></svg>'))
          plain t(".back_to_accounts")
        end
        h3(class: "text-sm font-semibold text-foreground") { t(".panel_title") }
        p(class: "text-xs text-muted-foreground mt-0.5") do
          plain t(".panel_description")
          span(class: "font-medium text-foreground") { @account.email_address }
          plain "."
        end
      end
    end

    def members_list
      div(class: "space-y-2") do
        @members.each { |member| member_row(member) }
      end
    end

    def member_row(member)
      div(class: "flex items-center gap-3 rounded-lg border border-border p-3") do
        render Campbooks::ContactAvatar.new(email: member.user.email_address, size: :md, variant: :accent)

        div(class: "min-w-0 flex-1") do
          div(class: "flex items-center gap-1.5") do
            span(class: "text-[13px] font-medium text-foreground truncate") { member.user.name.presence || member.user.email_address }
            span(class: "text-xs text-muted-foreground flex-shrink-0") { t(".you_badge") } if member.user_id == @current_user.id
          end
          p(class: "text-xs text-muted-foreground truncate") { member.user.email_address }
        end

        role_control(member)
      end
    end

    def role_control(member)
      if member.owner?
        render(Campbooks::Badge.new(variant: :accent, size: :md)) { t(".owner_badge") }
      else
        div(class: "flex items-center gap-1.5 flex-shrink-0") do
          role_select_form(member)
          remove_form(member)
        end
      end
    end

    def role_select_form(member)
      form(action: helpers.email_account_path(@account), method: "post", data: { controller: "auto-submit" }) do
        method_and_token
        input(type: "hidden", name: "user_email", value: member.user.email_address)
        select(name: "role", "aria-label": t(".role_for_aria", name: member.user.name),
               data: { action: "change->auto-submit#submit" }, class: SELECT_CLASSES) do
          EmailAccountUser::ROLES.each do |role|
            option(value: role, selected: member.role == role) { helpers.human_enum(EmailAccountUser, :role, role) }
          end
        end
      end
    end

    def remove_form(member)
      form(action: helpers.email_account_path(@account), method: "post",
           data: { turbo_confirm: t(".remove_confirm", name: member.user.name, email: @account.email_address) }) do
        method_and_token
        input(type: "hidden", name: "user_email", value: member.user.email_address)
        input(type: "hidden", name: "remove", value: "true")
        button(type: "submit", title: t(".remove_access_title"),
               class: "p-1.5 text-muted-foreground hover:text-red-600 hover:bg-red-50 dark:hover:bg-red-500/10 rounded-md transition-colors cursor-pointer") do
          raw(safe('<svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.8"><path stroke-linecap="round" stroke-linejoin="round" d="M19 7l-.8 12.1a2 2 0 01-2 1.9H7.8a2 2 0 01-2-1.9L5 7m5 4v6m4-6v6M9 7V4a1 1 0 011-1h4a1 1 0 011 1v3m-9 0h14"/></svg>'))
        end
      end
    end

    def add_person_section
      div(class: "rounded-lg border border-border bg-muted/30 p-4") do
        h4(class: "text-xs font-semibold text-foreground mb-3") { t(".add_section_title") }

        if @addable_users.empty?
          p(class: "text-xs text-muted-foreground") { t(".all_have_access") }
        else
          add_person_form
        end
      end
    end

    def add_person_form
      form(action: helpers.email_account_path(@account), method: "post", class: "flex flex-col sm:flex-row gap-2") do
        method_and_token
        select(name: "user_email", required: true, class: "#{SELECT_CLASSES} flex-1") do
          option(value: "") { t(".select_person_placeholder") }
          @addable_users.each do |user|
            option(value: user.email_address) { "#{user.name} (#{user.email_address})" }
          end
        end
        select(name: "role", "aria-label": t(".role_aria_label"), class: "#{SELECT_CLASSES} sm:w-36") do
          EmailAccountUser::ROLES.each do |role|
            option(value: role, selected: role == "viewer") { helpers.human_enum(EmailAccountUser, :role, role) }
          end
        end
        button(type: "submit",
               class: "inline-flex items-center justify-center rounded-lg bg-accent-600 hover:bg-accent-700 text-white text-xs font-medium px-4 py-1.5 cursor-pointer transition-colors") do
          plain t(".add_button")
        end
      end
    end

    def method_and_token
      input(type: "hidden", name: "_method", value: "patch")
      input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
    end
  end
end
