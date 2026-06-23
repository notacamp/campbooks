# frozen_string_literal: true

# Records that the current user has seen a one-time guided overlay ("tour"), so
# it won't greet them again. Fire-and-forget from the client — e.g. the skim
# intro's "Start skimming" button POSTs here. Unknown keys are accepted silently
# (the model just stores the string), keeping the endpoint forgiving for future
# tours added without a server change.
class ToursController < ApplicationController
  def dismiss
    Current.user&.dismiss_tour!(params[:key])
    head :no_content
  end
end
