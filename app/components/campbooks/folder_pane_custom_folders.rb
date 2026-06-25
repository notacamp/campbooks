# frozen_string_literal: true

module Campbooks
  # The folder pane's custom-folder rows, wrapped in #pane_custom_folders so the
  # create / update / destroy responses can re-render just this section live (the
  # system folders above never change on those actions). Each row carries a hover
  # "edit" affordance that opens a per-folder <dialog> to change its icon or delete
  # it; the dialogs live inside this section, so a save / delete that re-renders
  # #pane_custom_folders also closes the open dialog. Every row is a drag /
  # tap-to-move target.
  #
  # @param custom_folders [Array<MailFolder>]
  # @param current_folder [String, nil] active folder name (for the highlight)
  class FolderPaneCustomFolders < Campbooks::Base
    EDIT_SVG = '<svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.8" d="M16.862 4.487l1.687-1.688a1.875 1.875 0 1 1 2.652 2.652L10.582 16.07a4.5 4.5 0 0 1-1.897 1.13L6 18l.8-2.685a4.5 4.5 0 0 1 1.13-1.897l8.931-8.931z"/></svg>'
    CLOSE_SVG = '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/></svg>'

    def initialize(custom_folders:, current_folder: nil)
      @custom = custom_folders || []
      @current = current_folder
    end

    def view_template
      div(id: "pane_custom_folders", class: "space-y-0.5", data: { controller: "folder-edit" }) do
        if @custom.any?
          div(class: "mx-2 my-1.5 border-t border-border/60")
          @custom.each { |folder| folder_row(folder) }
        end
      end
    end

    private

    def folder_row(folder)
      active = @current == folder.name
      dialog_id = "edit_#{helpers.dom_id(folder)}"
      div(class: "group relative", id: helpers.dom_id(folder, :pane_folder)) do
        a(href: helpers.email_messages_path(folder_name: folder.name),
          data: { turbo_frame: "_top", folder_name: folder.name, folder_active: active.to_s, mail_folder_drop_target: "chip" },
          class: class_names(
            "flex items-center gap-2.5 rounded-lg py-1.5 pl-2.5 pr-8 transition-colors",
            active ? "bg-accent-50 font-medium text-accent-700 dark:bg-accent-500/15 dark:text-accent-200"
                   : "text-gray-600 hover:bg-muted dark:text-gray-300"
          )) do
          span(class: "flex h-5 w-5 flex-shrink-0 items-center justify-center") do
            render(Campbooks::Icon.new(folder.display_icon, css_class: "w-[18px] h-[18px]"))
          end
          span(class: "flex-1 truncate text-[13px]") { folder.name }
        end
        button(type: "button",
          data: { action: "folder-edit#open", folder_edit_dialog_param: dialog_id },
          aria_label: t("shared.actions.edit"),
          class: "absolute right-1 top-1/2 -translate-y-1/2 cursor-pointer rounded border-0 bg-transparent p-1 text-muted-foreground opacity-0 transition hover:bg-background/80 hover:text-foreground focus:opacity-100 group-hover:opacity-100") do
          raw safe(EDIT_SVG)
        end
        edit_dialog(folder, dialog_id)
      end
    end

    def edit_dialog(folder, dialog_id)
      dialog(id: dialog_id,
        class: "m-auto w-[calc(100vw-2rem)] max-w-sm overflow-hidden rounded-2xl border border-border bg-card p-0 text-card-foreground shadow-2xl backdrop:bg-black/40",
        data: { action: "click->folder-edit#backdropClose" }) do
        div(class: "flex items-start justify-between gap-3 border-b border-border px-5 py-4") do
          div do
            h2(class: "text-base font-semibold text-foreground") { t(".edit_title") }
            p(class: "mt-0.5 text-xs text-muted-foreground") { folder.name }
          end
          button(type: "button", aria_label: t("shared.actions.close"), data: { action: "folder-edit#close" },
            class: "-mr-1 cursor-pointer rounded border-0 bg-transparent p-1 text-gray-400 hover:bg-gray-200/70 hover:text-gray-600 dark:hover:bg-white/10 dark:hover:text-gray-200") do
            raw safe(CLOSE_SVG)
          end
        end

        form(action: helpers.mail_folder_path(folder), method: "post", accept_charset: "UTF-8") do
          input(type: "hidden", name: "_method", value: "patch")
          input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
          div(class: "space-y-1.5 px-5 py-5") do
            span(class: "block text-xs font-medium text-foreground") { t(".icon_label") }
            render(Campbooks::IconPicker.new(name: "mail_folder[icon]", selected: folder.icon))
          end
          div(class: "flex items-center justify-end border-t border-border px-5 py-3") do
            render(Campbooks::Button.new(variant: :primary, size: :sm, type: "submit")) { t("shared.actions.save") }
          end
        end

        # Delete — a separate sibling form (a <dialog> may hold several forms).
        form(action: helpers.mail_folder_path(folder), method: "post", accept_charset: "UTF-8",
          class: "border-t border-border px-5 py-3", data: { turbo_confirm: t(".delete_confirm", name: folder.name) }) do
          input(type: "hidden", name: "_method", value: "delete")
          input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
          button(type: "submit",
            class: "cursor-pointer border-0 bg-transparent text-xs font-medium text-red-600 hover:text-red-700 dark:text-red-400 dark:hover:text-red-300") do
            t("shared.actions.delete")
          end
        end
      end
    end
  end
end
