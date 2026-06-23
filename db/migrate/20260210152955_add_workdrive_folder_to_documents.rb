class AddWorkdriveFolderToDocuments < ActiveRecord::Migration[8.1]
  def change
    add_column :documents, :workdrive_folder, :string
  end
end
