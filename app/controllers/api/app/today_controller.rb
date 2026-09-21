# frozen_string_literal: true

module Api
  module App
    # GET /api/app/today
    #
    # The assistant-first Today surface: a ranked, finite "needs you" worklist
    # aggregated from People (PeopleStanding verb lanes), Money (reconciliation
    # + loan alerts, gated behind Features.accounting? + :accounting entitlement),
    # and Time (open asks + overdue deadlines). Also returns near-future coming_up
    # items and Scout's handled-this-week activity counts.
    #
    # READ-ONLY. No mutations. Each needs_you item carries source + ref_id +
    # action.kind so the client dispatches actions to the existing endpoints:
    #   - People: POST /api/app/people/:id/action
    #   - Money:  the reconciliation line-action endpoints
    #   - Time:   POST /api/app/asks/:id/done|snooze|hold|schedule etc.
    class TodayController < Api::App::BaseController
      def show
        result = ::Today::Aggregator.new(current_user, current_workspace).call
        render_data(
          Api::App::TodaySerializer.new(user: current_user, result: result).as_json
        )
      end
    end
  end
end
