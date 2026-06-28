# frozen_string_literal: true

module Api
  module V1
    # Serializes a MailFolder for the public API. List responses carry the core
    # fields; pass detail: true (the show endpoint) to include the filed documents.
    class FolderSerializer
      def initialize(folder, detail: false)
        @folder = folder
        @detail = detail
      end

      def as_json
        data = {
          id: @folder.id,
          name: @folder.name,
          icon: @folder.icon,
          parent_id: @folder.parent_id,
          position: @folder.position,
          document_count: @folder.folder_memberships.count,
          created_at: @folder.created_at.iso8601
        }

        if @detail
          data[:documents] = @folder.documents.order(created_at: :desc).map do |d|
            Api::V1::DocumentSerializer.new(d).as_json
          end
        end

        data
      end
    end
  end
end
