# Refreshes filter_data + tags on EmailMessage search records WITHOUT re-embedding.
# update_columns skips callbacks and leaves the (expensive) vectors untouched, so
# this is the cheap way to backfill the inbox-search filter keys onto records that
# were indexed before those keys existed. Enqueue in id-batches for large datasets:
#
#   SearchRecord.where(searchable_type: "EmailMessage").in_batches(of: 500) do |b|
#     BackfillEmailFilterDataJob.perform_later(b.pluck(:id))
#   end
class BackfillEmailFilterDataJob < ApplicationJob
  queue_as :default

  def perform(search_record_ids)
    SearchRecord.where(searchable_type: "EmailMessage", id: search_record_ids).find_each do |sr|
      msg = EmailMessage.find_by(id: sr.searchable_id)
      next unless msg

      sr.update_columns(
        filter_data: msg.searchable_filter_data,
        tags: msg.searchable_tags,
        source_updated_at: Time.current
      )
    end
  end
end
