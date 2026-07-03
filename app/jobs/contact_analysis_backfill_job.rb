# frozen_string_literal: true

# Periodically drains the backlog of never-analyzed contacts across every
# workspace (see Contacts::PendingAnalysisCatchUp). This is what makes contact
# profiling — and the Organizations directory built from it — self-heal after a
# mailbox is connected with existing history, or before a text-AI provider was
# configured, without anyone opening a page or running a manual backfill.
#
# Bounded per pass by the service's LIMIT (per workspace), so a large backlog
# drains gradually over successive ticks rather than flooding the queue. Cheap
# and idempotent once drained: the query returns nothing and no jobs are enqueued.
class ContactAnalysisBackfillJob < ApplicationJob
  queue_as :default

  def perform
    Workspace.find_each { |workspace| Contacts::PendingAnalysisCatchUp.run(workspace) }
  end
end
