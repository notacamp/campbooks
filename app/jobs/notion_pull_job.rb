class NotionPullJob < ApplicationJob
  queue_as :default

  retry_on Faraday::Error, wait: :polynomially_longer, attempts: 3

  def perform(mapping_id)
    mapping = NotionDatabaseMapping.find(mapping_id)
    result = Notion::PullService.new(mapping).call

    Rails.logger.info("[NotionPullJob] Pull complete for mapping #{mapping_id}: #{result.inspect}")

    result
  end
end
