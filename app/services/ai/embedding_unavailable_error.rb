# frozen_string_literal: true

module Ai
  # Raised by Search::WorkspaceReembedJob when EmbeddingService.embed_batch
  # returns blank mid-sweep for a workspace that was configured at the start
  # of the run. This means the provider became unavailable (quota exhausted,
  # adapter dropped) after the job's entry guard passed.
  #
  # Unlike transient Faraday errors (TRANSIENT_ERRORS), this is NOT included
  # in the retry_on list — the job lands in the failed set where an operator
  # can inspect it, rather than retrying invisibly and masking the outage.
  class EmbeddingUnavailableError < StandardError; end
end
