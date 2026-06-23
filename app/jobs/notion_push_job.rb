class NotionPushJob < ApplicationJob
  queue_as :default

  retry_on Faraday::Error, wait: :polynomially, attempts: 3
  retry_on RuntimeError, wait: :polynomially, attempts: 3

  def perform(document_id, mapping_id = nil)
    document = Document.find(document_id)
    mapping = mapping_id ? NotionDatabaseMapping.find(mapping_id) : nil

    result = Notion::PushService.new(document, mapping).call

    unless result[:success]
      Rails.logger.error("[NotionPushJob] Push failed for document #{document_id}: #{result[:error]}")
    end

    result
  end
end
