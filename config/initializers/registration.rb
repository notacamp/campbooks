# frozen_string_literal: true

# Registration / signup gating.
#
# SIGNUP_MODE controls who may create a brand-new account (a new Workspace) on
# this instance. Invited users always bypass the gate — an invitation is its
# own authorization.
#
#   open        – anyone with a valid email can sign up (default for self-hosted)
#   beta_code   – anyone who also enters a valid single-use invite code (default for cloud)
#   approval    – signups land in the admin approval queue (the original closed beta)
#   invite_only – no public signup at all; an invitation is required
#
# Beta invite codes are single-use, stored in the DB (BetaCode model) and minted
# by admins at /admin/beta_codes — there is no shared-secret env var.
#
# Defaults: self-hosted → :open, cloud (hosted) → :beta_code. Override with the
# SIGNUP_MODE env var. self_hosted.rb has not run yet at this point, so read the
# SELF_HOSTED env var directly rather than Rails.application.config.self_hosted.
self_hosted = ENV["SELF_HOSTED"].present?

Rails.application.config.signup_mode =
  (ENV["SIGNUP_MODE"].presence || (self_hosted ? "open" : "beta_code")).to_sym
