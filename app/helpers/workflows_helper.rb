module WorkflowsHelper
  # Maps a workflow execution / step status to a Campbooks::Badge variant.
  def execution_badge_variant(record)
    case record.status.to_s
    when "completed" then :success
    when "failed"    then :danger
    when "running"   then :info
    when "skipped"   then :neutral
    else :warning
    end
  end
end
