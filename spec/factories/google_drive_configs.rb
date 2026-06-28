FactoryBot.define do
  factory :google_drive_config do
    document_type
    auto_push { true }
    folder_id { "test-folder-id" }
    folder_path { "Campbooks / Test" }
    naming_pattern { "{date}_{entity}_{reference}" }
    subfolder_pattern { "flat" }
  end
end
