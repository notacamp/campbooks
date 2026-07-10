# frozen_string_literal: true

module Campbooks
  module Files
    # An internal (authored) document as a list item in the Files area. Responsive
    # flex row (works desktop + mobile). Links to the in-app editor; the kebab opens
    # / edits / files it into a folder / removes it from the current folder. Folder
    # mutations submit non-Turbo (FolderMembershipsController redirects back).
    class DocRow < Campbooks::Base
      def initialize(doc:, folders: [], current_folder: nil)
        @doc = doc
        @folders = folders || []
        @current_folder = current_folder
      end

      def view_template
        div(id: helpers.dom_id(@doc),
          class: "flex items-center gap-3 rounded-xl border border-gray-200 bg-card px-4 py-3 shadow-sm dark:border-white/10") do
          # Inside the `files_results` frame — break out (see FileRow).
          a(href: helpers.written_document_path(@doc), class: "flex min-w-0 flex-1 items-center gap-3", data: { turbo_frame: "_top" }) do
            span(class: "flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-lg bg-ember/10 text-ember") { raw(safe(DOC_ICON)) }
            span(class: "min-w-0 flex-1") do
              span(class: "block truncate text-sm font-medium text-foreground") { @doc.title }
              span(class: "mt-0.5 block truncate text-xs text-muted-foreground") { meta }
            end
          end
          menu
        end
      end

      private

      def meta
        "#{t('.kind')} · #{t('.updated', date: l(@doc.updated_at.to_date, format: :long))}"
      end

      def menu
        details(class: "relative inline-block flex-shrink-0 text-left", data: { controller: "dropdown-close" }) do
          summary(class: "inline-flex h-8 w-8 cursor-pointer list-none items-center justify-center rounded-md text-muted-foreground hover:bg-muted [&::-webkit-details-marker]:hidden",
            aria: { label: t(".actions") }) { raw(safe(DOTS_ICON)) }
          div(class: "absolute right-0 z-20 mt-1 w-52 rounded-lg border border-border bg-card p-1 text-left shadow-lg") do
            a(href: helpers.written_document_path(@doc), class: menu_item, data: { turbo_frame: "_top" }) { t(".open") }
            a(href: helpers.edit_written_document_path(@doc), class: menu_item, data: { turbo_frame: "_top" }) { t(".edit") }
            move_section
            remove_item if @current_folder
          end
        end
      end

      def move_section
        targets = @folders.reject { |f| @current_folder && f.id == @current_folder.id }
        return if targets.empty?

        div(class: "my-1 border-t border-border")
        p(class: "px-2 py-1 text-[11px] font-semibold uppercase tracking-wide text-muted-foreground") { t(".move_to") }
        targets.each do |folder|
          membership_form(helpers.folder_memberships_path, :post,
            { mail_folder_id: folder.id, folderable_id: @doc.id, folderable_type: "AuthoredDocument" }, folder.name)
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
          button(type: "submit", class: menu_item) { label }
        end
      end

      def menu_item
        "block w-full cursor-pointer rounded-md px-2 py-1.5 text-left text-sm text-foreground hover:bg-muted"
      end

      DOC_ICON = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" class="h-[18px] w-[18px]"><path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z"/><path d="M14 3v5h5M9 13h6M9 17h4"/></svg>'
      DOTS_ICON = '<svg viewBox="0 0 24 24" fill="currentColor" class="h-4 w-4"><circle cx="12" cy="5" r="1.6"/><circle cx="12" cy="12" r="1.6"/><circle cx="12" cy="19" r="1.6"/></svg>'
    end
  end
end
