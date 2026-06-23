class NotionSyncJob < ApplicationJob
  queue_as :default

  def perform
    # A workspace may now connect several Notion workspaces; sync each Campbooks
    # workspace once (the legacy auto-sync mappings resolve their own integration).
    NotionIntegration.active.distinct.pluck(:workspace_id).compact.each do |org_id|
      # Pull from databases with pull enabled
      NotionDatabaseMapping.joins(:document_type)
        .where(document_types: { workspace_id: org_id }, pull_enabled: true)
        .find_each do |mapping|
        NotionPullJob.perform_later(mapping.id)
      end

      # Push outdated documents
      NotionPage.outdated.joins(:document)
        .where(documents: { workspace_id: org_id })
        .find_each do |np|
        NotionPushJob.perform_later(np.document_id, np.notion_database_mapping_id)
      end
    end
  end
end
