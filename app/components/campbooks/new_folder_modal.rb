# frozen_string_literal: true

module Campbooks
  # "New folder" dialog for the inbox folder bar. A native <dialog> whose form
  # POSTs to MailFoldersController#create; on success the create response appends
  # the new chip + a toast, and the `new-folder` Stimulus controller closes the
  # dialog. On a validation error the response fills #new_folder_error and the
  # dialog stays open.
  #
  # Lives inside the folder bar's `new-folder` controller scope (the "+" trigger
  # there carries `data-action="new-folder#open"`), so it needs no trigger of its
  # own. The single name field is enough — creating a folder is one decision.
  class NewFolderModal < Campbooks::Base
    CLOSE_SVG = '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/></svg>'

    # @param open [Boolean] render the dialog already open (Lookbook preview only).
    # @param error [String, nil] preset validation error (preview only).
    def initialize(open: false, error: nil)
      @open = open
      @error = error
    end

    def view_template
      dialog(
        id: "new-folder-dialog",
        class: "rounded-2xl shadow-2xl border border-border p-0 overflow-hidden backdrop:bg-black/40 m-auto " \
               "w-[calc(100vw-2rem)] max-w-md bg-card text-card-foreground",
        aria: { label: t(".title") },
        open: @open ? "" : nil,
        data: {
          new_folder_target: "dialog",
          action: "click->new-folder#backdropClose"
        }
      ) do
        form(
          action: helpers.mail_folders_path,
          method: "post",
          accept_charset: "UTF-8",
          data: { new_folder_target: "form", action: "turbo:submit-end->new-folder#submitEnd" }
        ) do
          input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
          header
          body
          footer
        end
      end
    end

    private

    def header
      div(class: "flex items-start justify-between gap-3 border-b border-border px-5 py-4") do
        div do
          h2(class: "text-base font-semibold text-foreground") { t(".title") }
          p(class: "mt-0.5 text-xs text-muted-foreground") { t(".subtitle") }
        end
        button(
          type: "button",
          aria_label: t("shared.actions.close"),
          class: "p-1 -mr-1 rounded text-gray-400 hover:bg-gray-200/70 hover:text-gray-600 dark:hover:bg-white/10 dark:hover:text-gray-200 transition-colors cursor-pointer border-0 bg-transparent",
          data: { action: "click->new-folder#close" }
        ) { raw(safe(CLOSE_SVG)) }
      end
    end

    def body
      div(class: "px-5 py-5 space-y-4") do
        render(Campbooks::Input.new(
          "mail_folder[name]",
          label: t(".name_label"),
          placeholder: t(".name_placeholder"),
          required: true,
          autocomplete: "off",
          data: { new_folder_target: "input", action: "input->new-folder#clearError" }
        ))
        div(class: "space-y-1.5") do
          span(class: "block text-xs font-medium text-foreground") { t(".icon_label") }
          render(Campbooks::IconPicker.new(name: "mail_folder[icon]"))
        end
        # Turbo Stream target the server fills with a validation error (kept open).
        div(id: "new_folder_error") do
          if @error
            p(class: "text-sm font-medium text-red-600 dark:text-red-400", role: "alert") { @error }
          end
        end
      end
    end

    def footer
      div(class: "flex items-center justify-end gap-2 border-t border-border px-5 py-3") do
        render(Campbooks::Button.new(
          variant: :ghost, size: :sm, type: "button",
          data: { action: "click->new-folder#close" }
        )) { t("shared.actions.cancel") }
        render(Campbooks::Button.new(variant: :primary, size: :sm, type: "submit")) { t(".submit") }
      end
    end
  end
end
