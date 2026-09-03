# frozen_string_literal: true

module Campbooks
  module Paper
    # The per-document kebab on a Paper row: Open, Download, Mark paid/unpaid (money types),
    # Send to Drive, Send to Notion, Move to folder, Reprocess, and Delete (manual uploads).
    # Mirrors Campbooks::Files::FileActionsMenu (details/summary + dropdown-close), adding the
    # settlement toggle and the Drive/Notion exports the Rethink moves into the row menu.
    class RowMenu < Campbooks::Base
      DOTS_ICON = '<svg viewBox="0 0 24 24" fill="currentColor" class="h-4 w-4"><circle cx="12" cy="5" r="1.6"/><circle cx="12" cy="12" r="1.6"/><circle cx="12" cy="19" r="1.6"/></svg>'

      def initialize(document:, folders: [])
        @doc = document
        @folders = folders || []
      end

      def view_template
        details(class: "relative inline-block text-left", data: { controller: "dropdown-close" }) do
          summary(class: "inline-flex h-8 w-8 cursor-pointer list-none items-center justify-center rounded-md text-muted-foreground hover:bg-muted [&::-webkit-details-marker]:hidden",
            aria: { label: t("components.paper.row_menu.actions") }) { raw(safe(DOTS_ICON)) }
          div(class: "absolute right-0 z-20 mt-1 w-56 rounded-lg border border-border bg-card p-1 text-left shadow-lg") do
            menu_link(helpers.document_path(@doc), t("components.paper.row_menu.open"))
            menu_link(helpers.file_document_path(@doc, disposition: "attachment"), t("components.paper.row_menu.download"))
            settle_item
            divider
            menu_link(helpers.new_document_drive_export_path(@doc), t("components.paper.row_menu.send_to_drive"))
            menu_link(helpers.new_document_notion_export_path(@doc), t("components.paper.row_menu.send_to_notion"))
            move_section
            divider
            reprocess_item
            delete_item if @doc.manual_upload?
          end
        end
      end

      private

      # Money documents get the paid/unpaid toggle. A bank-match settlement belongs to the
      # reconciliation (not the row), so it isn't offered a manual "unpaid".
      def settle_item
        return unless @doc.document_type.in?(Document::MONEY_TYPES)

        if !@doc.settled?
          action_form(helpers.settle_document_path(@doc), :post, t("components.paper.row_menu.mark_paid"))
        elsif @doc.settled_manual?
          action_form(helpers.unsettle_document_path(@doc), :delete, t("components.paper.row_menu.mark_unpaid"))
        end
      end

      def move_section
        return if @folders.empty?

        divider
        p(class: "px-2 py-1 text-[11px] font-semibold uppercase tracking-wide text-muted-foreground") { t("components.paper.row_menu.move_to") }
        @folders.each do |folder|
          membership_form(helpers.folder_memberships_path,
            { mail_folder_id: folder.id, folderable_id: @doc.id }, folder.name)
        end
      end

      def reprocess_item
        # reprocess redirects to the document (not a Turbo Stream), so navigate normally.
        form(action: helpers.reprocess_document_path(@doc), method: "post", class: "block", data: { turbo: false }) do
          token_field
          button(type: "submit", class: menu_item_classes) { t("components.paper.row_menu.reprocess") }
        end
      end

      def delete_item
        divider
        form(action: helpers.files_upload_path(@doc), method: "post", class: "block",
          data: { turbo_confirm: t("components.paper.row_menu.delete_confirm", name: @doc.display_title) }) do
          token_field
          method_field(:delete)
          button(type: "submit", class: class_names(menu_item_classes, "text-red-600 hover:bg-red-50 dark:text-red-400 dark:hover:bg-red-500/10")) do
            t("components.paper.row_menu.delete")
          end
        end
      end

      # A Turbo form (settle/unsettle stream back the refreshed row + an undo toast).
      def action_form(url, method, label)
        form(action: url, method: "post", class: "block") do
          token_field
          method_field(method) unless method == :post
          button(type: "submit", class: menu_item_classes) { label }
        end
      end

      # Folder moves submit non-Turbo (FolderMembershipsController redirects back).
      def membership_form(url, fields, label)
        form(action: url, method: "post", class: "block", data: { turbo: false }) do
          token_field
          fields.each { |name, value| input(type: "hidden", name: name.to_s, value: value.to_s) }
          button(type: "submit", class: menu_item_classes) { label }
        end
      end

      def token_field
        input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
      end

      def method_field(method)
        input(type: "hidden", name: "_method", value: method.to_s)
      end

      def divider
        div(class: "my-1 border-t border-border")
      end

      # Menu links leave the list (document page / download / export wizard) — break out of
      # the paper_results frame or Turbo shows "Content missing".
      def menu_link(href, label)
        a(href: href, class: menu_item_classes, data: { turbo_frame: "_top" }) { label }
      end

      def menu_item_classes
        "block w-full cursor-pointer rounded-md px-2 py-1.5 text-left text-sm text-foreground hover:bg-muted"
      end
    end
  end
end
