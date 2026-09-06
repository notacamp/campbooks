# frozen_string_literal: true

module Asks
  # Hand an ask to a teammate — the one action that survives from the retired task
  # module. Replaces whatever assignments the ask carried with exactly one (so
  # Task#handed_to is unambiguous), accepts the ask (a suggested one becomes open
  # on hand-off — you don't hand over a guess), records the domain event, and
  # notifies the assignee with an action-required notice that lands on their Now.
  #
  # Taking it back is the inverse and lives in AsksController#take_back (it needs
  # the acting user's authority check), reusing Task#resolve — style resolution to
  # clear the assignee's notice.
  module HandOff
    module_function

    # @param ask [Task]
    # @param to  [User] a workspace member other than `by`
    # @param by  [User] the acting user handing it over
    # @return [Task] the handed ask
    def call(ask, to:, by:)
      ask.transaction do
        ask.task_assignments.destroy_all
        ask.task_assignments.create!(user: to, assigned_by: by)
        ask.accept!(by: by)
      end

      Events.publish("task.handed_off", subject: ask, actor: by,
                     payload: { title: ask.title, to_user_id: to.id, by_user_id: by.id })
      Notifier.task_handed_off(ask, to: to, by: by)
      ask
    end
  end
end
