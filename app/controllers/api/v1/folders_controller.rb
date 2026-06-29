# frozen_string_literal: true

module Api
  module V1
    # Lists and shows workspace custom folders (MailFolder). Read-only over the API;
    # create / rename / delete have provider side-effects and are not exposed here.
    class FoldersController < BaseController
      before_action -> { doorkeeper_authorize! :"folders:read" }, only: [ :index, :show ]

      def index
        folders = Current.workspace.mail_folders.ordered
        render_data(folders.map { |f| FolderSerializer.new(f).as_json })
      end

      def show
        folder = Current.workspace.mail_folders.find(params[:id])
        render_data(FolderSerializer.new(folder, detail: true).as_json)
      end
    end
  end
end
