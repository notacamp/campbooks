class AccountDeletionJob < ApplicationJob
  queue_as :default

  def perform(user_id)
    user = User.find_by(id: user_id)
    return unless user

    Accounts::Deleter.new(user).delete!
  end
end
