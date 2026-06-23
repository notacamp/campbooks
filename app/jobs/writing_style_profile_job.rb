class WritingStyleProfileJob < ApplicationJob
  queue_as :default

  def perform(user_id)
    user = User.find_by(id: user_id)
    return unless user

    # No session in a background job — set the acting identity so
    # Ai::Configuration.for (Current.workspace) resolves the workspace's model.
    Current.workspace = user.workspace
    Current.acting_user = user

    Ai::WritingStyleProfiler.call(user)
  end
end
