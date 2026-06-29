# frozen_string_literal: true

module Campbooks
  module Files
    # The Files area folder rail (desktop, lg+). An "All files" root plus the
    # workspace's custom folders as a nested tree, each linking to the folder's
    # Files view (files_folder_path), with a file-count badge. The "New folder"
    # button opens the shared Campbooks::NewFolderModal (rendered by the page within
    # a `new-folder` Stimulus scope). Folder rename/move/delete stays in the Mail
    # folder pane for now — these are the same unified MailFolder records.
    class Sidebar < Campbooks::Base
      # @param folders [Array<MailFolder>] the workspace's custom folders (flat)
      # @param current_folder [MailFolder, nil] the folder being viewed (highlight)
      # @param counts [Hash] { mail_folder_id => Integer } file counts
      def initialize(folders:, current_folder: nil, counts: {})
        @folders = folders || []
        @current = current_folder
        @counts = counts || {}
        @children_by_parent = @folders.group_by(&:parent_id)
      end

      def view_template
        nav(class: "w-56 flex-shrink-0", aria: { label: t(".label") }) do
          all_files_link
          div(class: "mt-1 space-y-0.5") do
            (@children_by_parent[nil] || []).each { |folder| subtree(folder, 0) }
          end
          new_folder_button
        end
      end

      private

      def all_files_link
        a(href: helpers.files_path, class: row_classes(@current.nil?)) do
          span(class: "flex h-5 w-5 flex-shrink-0 items-center justify-center") { raw(safe(ALL_ICON)) }
          span(class: "truncate") { t(".all_files") }
        end
      end

      def subtree(folder, depth)
        folder_row(folder, depth)
        (@children_by_parent[folder.id] || []).each { |child| subtree(child, depth + 1) }
      end

      def folder_row(folder, depth)
        active = @current&.id == folder.id
        a(href: helpers.files_folder_path(folder), class: row_classes(active),
          style: "padding-left: #{(0.5 + depth * 0.875).round(3)}rem") do
          span(class: "flex h-5 w-5 flex-shrink-0 items-center justify-center") do
            render(Campbooks::Icon.new(folder.display_icon, css_class: "w-[18px] h-[18px]"))
          end
          span(class: "truncate flex-1") { folder.name }
          count_badge(folder, active)
        end
      end

      def count_badge(folder, active)
        count = @counts[folder.id].to_i
        return if count.zero?

        span(class: class_names(
          "ml-auto inline-flex h-4 min-w-[18px] items-center justify-center rounded-full px-1 text-[10px] font-bold",
          active ? "bg-accent-600 text-white" : "bg-muted text-muted-foreground"
        )) { count.to_s }
      end

      def new_folder_button
        button(type: "button", data: { action: "new-folder#open" },
          class: "mt-2 flex w-full cursor-pointer items-center gap-2 rounded-lg px-2 py-1.5 text-[13px] text-muted-foreground hover:bg-muted") do
          span(class: "flex h-5 w-5 flex-shrink-0 items-center justify-center") { raw(safe(PLUS_ICON)) }
          span { t(".new_folder") }
        end
      end

      def row_classes(active)
        class_names(
          "flex items-center gap-2.5 rounded-lg px-2 py-1.5 text-[13px]",
          active ? "bg-accent-50 font-medium text-accent-700 dark:bg-accent-500/15 dark:text-accent-200" \
                 : "text-gray-600 hover:bg-muted dark:text-gray-300"
        )
      end

      ALL_ICON = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" class="w-[18px] h-[18px]"><path d="M4 6h16M4 12h16M4 18h16"/></svg>'
      PLUS_ICON = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" class="w-4 h-4"><path d="M12 5v14M5 12h14"/></svg>'
    end
  end
end
