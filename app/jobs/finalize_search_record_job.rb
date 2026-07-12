class FinalizeSearchRecordJob < ApplicationJob
  queue_as :default
  queue_with_priority BACKGROUND_PRIORITY
  # The last chunks of one searchable finish embedding near-simultaneously, so
  # several finalize jobs can race on the unique (searchable_type, searchable_id)
  # index. The loser retries and updates the now-committed row via find_or_initialize.
  retry_on ActiveRecord::RecordNotUnique, wait: 2.seconds, attempts: 3

  def perform(searchable_type, searchable_id)
    searchable = searchable_type.constantize.find_by(id: searchable_id)
    return unless searchable

    # Delegates all finalization logic to the shared service so that
    # WorkspaceReembedJob can reuse it with precomputed (batched) title vectors.
    Search::RecordFinalizer.call(searchable)
  end
end
