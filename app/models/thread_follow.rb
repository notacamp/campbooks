# A user following a discussion thread. Drives two things: who gets notified of
# new activity, and (for teammates pulled in by an @mention without mailbox
# access) who is granted access to the thread. See EmailThread#accessible_by?.
class ThreadFollow < ApplicationRecord
  belongs_to :user
  belongs_to :agent_thread

  validates :user_id, uniqueness: { scope: :agent_thread_id }
end
