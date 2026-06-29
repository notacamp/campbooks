# frozen_string_literal: true

module Campbooks
  module Files
    # The per-file kebab menu shared by FileRow and FileCard: open, download,
    # move-to-folder, remove-from-folder, delete. Folder mutations submit non-Turbo
    # (data-turbo:false) so they follow FolderMembershipsController's html
    # redirect_back rather than its documents-page Turbo stream. Delete posts to
    # Files::UploadsController#destroy (redirect-only), so it stays a Turbo form
    # with a confirm. Closes on outside click via the `dropdown-close` controller.
    class FileActionsMenu < Campbooks::Base
      def initialize(doc:, folders: [], current_folder: nil)
        @doc = doc
        @folders = folders || []
        @current_folder = current_folder
      end

      def view_template
        details(class: "relative inline-block text-left", data: { controller: "dropdown-close" }) do
          summary(class: "inline-flex h-8 w-8 cursor-pointer list-none items-center justify-center rounded-md text-muted-foreground hover:bg-muted [&::-webkit-details-marker]:hidden",
            aria: { label: t(".actions") }) { raw(safe(DOTS_ICON)) }
          div(class: "absolute right-0 z-20 mt-1 w-52 rounded-lg border border-border bg-card p-1 text-left shadow-lg") do
            menu_link(helpers.document_path(@doc), t(".open"))
            menu_link(helpers.file_document_path(@doc, disposition: "attachment"), t(".download"))
            move_section
            remove_item if @current_folder
            delete_item if @doc.manual_upload?
          end
        end
      end

      private

      def move_section
        targets = @folders.reject { |f| @current_folder && f.id == @current_folder.id }
        return if targets.empty?

        div(class: "my-1 border-t border-border")
        p(class: "px-2 py-1 text-[11px] font-semibold uppercase tracking-wide text-muted-foreground") { t(".move_to") }
        targets.each do |folder|
          membership_form(helpers.folder_memberships_path, :post,
            { mail_folder_id: folder.id, folderable_id: @doc.id }, folder.name)
        end
      end

      def remove_item
        membership = @current_folder.folder_memberships.find_by(folderable: @doc)
        return unless membership

        membership_form(helpers.folder_membership_path(membership), :delete, {}, t(".remove_from_folder"))
      end

      def membership_form(url, method, fields, label)
        form(action: url, method: "post", class: "block", data: { turbo: false }) do
          input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
          input(type: "hidden", name: "_method", value: method.to_s) unless method == :post
          fields.each { |name, value| input(type: "hidden", name: name.to_s, value: value.to_s) }
          button(type: "submit", class: menu_item_classes) { label }
        end
      end

      def delete_item
        div(class: "my-1 border-t border-border")
        form(action: helpers.files_upload_path(@doc), method: "post", class: "block",
          data: { turbo_confirm: t(".delete_confirm", name: @doc.display_title) }) do
          input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
          input(type: "hidden", name: "_method", value: "delete")
          button(type: "submit", class: class_names(menu_item_classes, "text-red-600 hover:bg-red-50 dark:text-red-400 dark:hover:bg-red-500/10")) { t(".delete") }
        end
      end

      def menu_link(href, label)
        a(href: href, class: menu_item_classes) { label }
      end

      def menu_item_classes
        "block w-full cursor-pointer rounded-md px-2 py-1.5 text-left text-sm text-foreground hover:bg-muted"
      end

      DOTS_ICON = '<svg viewBox="0 0 24 24" fill="currentColor" class="h-4 w-4"><circle cx="12" cy="5" r="1.6"/><circle cx="12" cy="12" r="1.6"/><circle cx="12" cy="19" r="1.6"/></svg>'
    end
  end
end
