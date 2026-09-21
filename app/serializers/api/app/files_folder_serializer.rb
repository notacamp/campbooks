# frozen_string_literal: true

module Api
  module App
    # Screen-shaped read model for the Files folder view. Returns the folder tree
    # (for the sidebar) plus the current folder's paginated contents.
    class FilesFolderSerializer
      def initialize(folders:, current_folder: nil, documents: [], pagy: nil, folder_counts: {})
        @folders        = folders
        @current_folder = current_folder
        @documents      = documents
        @pagy           = pagy
        @folder_counts  = folder_counts
      end

      def as_json
        {
          folders:        serialize_tree,
          current_folder: current_folder_data,
          files:          serialize_documents,
          meta:           meta
        }
      end

      private

      def serialize_tree
        @folders.map do |folder|
          {
            id:       folder.id,
            name:     folder.name,
            icon:     folder.try(:icon),
            parent_id: folder.parent_id,
            depth:    folder.try(:depth) || 0,
            position: folder.position,
            counts:   @folder_counts[folder.id] || { files: 0, emails: 0, total: 0 }
          }
        end
      end

      def current_folder_data
        return nil unless @current_folder

        {
          id:       @current_folder.id,
          name:     @current_folder.name,
          icon:     @current_folder.try(:icon),
          parent_id: @current_folder.parent_id
        }
      end

      def serialize_documents
        @documents.map { |doc| Api::App::DocumentSerializer.new(doc).as_json }
      end

      def meta
        return nil unless @pagy

        {
          page:        @pagy.page,
          per_page:    @pagy.limit,
          total:       @pagy.count,
          total_pages: @pagy.pages
        }
      end
    end
  end
end
