# OpenSearch powers the Contact full-text index (Searchkick). It is OPTIONAL:
# email/document search and the Cmd+K palette use pgvector in Postgres, and
# Contact search itself falls back to SQL ILIKE when the cluster is absent.
#
# Resolve the cluster URL from OPENSEARCH_URL. In development/test we keep the
# historical localhost default so nothing changes for contributors. In
# production (e.g. a lightweight self-hosted box without OpenSearch) an unset
# OPENSEARCH_URL means "no search cluster" — so we disable Searchkick callbacks
# rather than enqueue Contact reindex jobs that could never succeed.
opensearch_url = ENV.fetch("OPENSEARCH_URL") { Rails.env.local? ? "http://localhost:9200" : nil }

if opensearch_url.present?
  Searchkick.client = OpenSearch::Client.new(
    url: opensearch_url,
    retry_on_failure: 3,
    request_timeout: 5
  )
else
  Searchkick.disable_callbacks
end

# Reindex callbacks run through Active Job (:async) — configured per-model on the
# Contact `searchkick` macro — so Contact writes never block on or fail with
# OpenSearch. See app/models/contact.rb.
