# Be sure to restart your server when you modify this file.

# Define an application-wide content security policy.
# See the Securing Rails Applications Guide for more information:
# https://guides.rubyonrails.org/security.html#content-security-policy-header

# Rails.application.configure do
#   config.content_security_policy do |policy|
#     policy.default_src :self, :https
#     policy.font_src    :self, :https, :data
#     policy.img_src     :self, :https, :data
#     policy.object_src  :none
#     policy.script_src  :self, :https
#     policy.style_src   :self, :https
#     # Specify URI for violation reports
#     # policy.report_uri "/csp-violation-report-endpoint"
#   end
#
#   # Generate session nonces for permitted importmap, inline scripts, and inline styles.
#   config.content_security_policy_nonce_generator = ->(request) { request.session.id.to_s }
#   config.content_security_policy_nonce_directives = %w(script-src style-src)
#
#   # Automatically add `nonce` to `javascript_tag`, `javascript_include_tag`, and `stylesheet_link_tag`
#   # if the corresponding directives are specified in `content_security_policy_nonce_directives`.
#   # config.content_security_policy_nonce_auto = true
#
#   # Report violations without enforcing the policy.
#   # config.content_security_policy_report_only = true
# end

# Baseline Content-Security-Policy. These directives need no nonce and cannot
# break inline scripts, so they are safe to enforce now: they shut down plugin/
# object injection, <base> hijacking, clickjacking, and cross-origin form posting.
#
# NOTE: a full `script-src 'self'` + per-request nonce (the real reflected/stored
# XSS backstop) is a deliberate follow-up — it requires noncing every inline
# <script> and Playwright-verifying each page. The known stored-XSS sinks are
# already neutralised at the source: email bodies via Loofah :prune
# (EmailMessageHelpers), Scout/AI output via Redcarpet `filter_html`, and SVG
# attachments served sandboxed + nosniff (EmailImagesController).
Rails.application.configure do
  config.content_security_policy do |policy|
    policy.object_src      :none
    policy.base_uri        :self
    policy.frame_ancestors :self
    # 'self' plus the mailbox OAuth providers: connecting Google/Zoho/Microsoft
    # POSTs to /email_accounts and the server 302s to the provider's consent
    # screen. Browsers enforce form-action across the POST's redirect chain, so
    # without these origins the redirect is blocked and "Connect …" silently does
    # nothing (the provider auth hosts, matching the OauthClient AUTH_URLs).
    # Zoho's data center is region-configurable (ZOHO_REGION). Resolve its accounts
    # host from ENV here — this initializer runs before Zeitwerk can autoload
    # Zoho::Region, so we can't reference it. Keep this map in sync with
    # Zoho::Region::DOMAINS.
    zoho_accounts_domain = {
      "eu" => "zoho.eu", "us" => "zoho.com", "com" => "zoho.com", "in" => "zoho.in",
      "au" => "zoho.com.au", "com.au" => "zoho.com.au", "jp" => "zoho.jp",
      "ca" => "zohocloud.ca", "cn" => "zoho.com.cn", "sa" => "zoho.sa"
    }.fetch(ENV.fetch("ZOHO_REGION", "eu").to_s.strip.downcase, "zoho.eu")

    policy.form_action :self,
      "https://accounts.google.com",
      "https://accounts.#{zoho_accounts_domain}",
      "https://login.microsoftonline.com"

    # Report violations of the enforced policy above (object/base/frame/form) to an
    # internal endpoint so they surface in logs/GlitchTip. Cheap: these directives
    # rarely trip, so this is low-volume telemetry, not noise.
    policy.report_uri "/csp-reports"
  end
end

Rails.application.config.action_dispatch.default_headers.merge!(
  "X-Content-Type-Options" => "nosniff",
  "Referrer-Policy" => "strict-origin-when-cross-origin",
  "Permissions-Policy" => "camera=(), microphone=(), geolocation=()"
)

# Report-only `script-src 'self'` observation — OFF by default, opt-in via
# CSP_REPORT_ONLY_SCRIPT_SRC=1. Emits a SECOND, non-enforcing
# Content-Security-Policy-Report-Only header so we can collect the exact inventory
# of inline scripts (app + Matomo/Chatwoot overlays + Turbo/importmap) that a
# future enforced `script-src 'self'` + nonce would block — WITHOUT breaking
# anything. Flip on in prod to gather reports, then off. (Report-only never blocks.)
if ENV["CSP_REPORT_ONLY_SCRIPT_SRC"].to_s == "1"
  Rails.application.config.action_dispatch.default_headers.merge!(
    "Content-Security-Policy-Report-Only" => "script-src 'self' 'report-sample'; report-uri /csp-reports"
  )
end
