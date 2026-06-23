# Be sure to restart your server when you modify this file.

# Configure parameters to be partially matched (e.g. passw matches password) and filtered from the log file.
# Use this to limit dissemination of sensitive information.
# See the ActiveSupport::ParameterFilter documentation for supported notations and behaviors.
Rails.application.config.filter_parameters += [
  # Credentials / secrets. :token already covers access_token / refresh_token;
  # :code catches the OAuth authorization code plus MFA / recovery one-time codes.
  :passw, :secret, :token, :_key, :crypt, :salt, :certificate, :otp, :cvv, :cvc,
  :code, :code_verifier,
  # User data / PII — email content, names, contact + financial + free-text fields,
  # and search terms. (:address covers to_/cc_/from_/email_/ip_address; :name
  # covers first_/last_/vendor_/client_/display_name.)
  :email, :body, :content, :message, :body_html, :body_text, :snippet,
  :name, :subject, :address, :recipient, :phone, :ssn, :nif, :vat, :iban,
  :account_number, :amount, :writing_style, :description, :query, :search
]
