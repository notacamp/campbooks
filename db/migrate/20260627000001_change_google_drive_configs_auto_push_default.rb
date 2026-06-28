class ChangeGoogleDriveConfigsAutoPushDefault < ActiveRecord::Migration[8.1]
  def change
    change_column_default :google_drive_configs, :auto_push, from: false, to: true
  end
end
