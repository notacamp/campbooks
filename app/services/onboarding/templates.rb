# frozen_string_literal: true

module Onboarding
  # Persona-based setup templates that tailor a new workspace to how the user
  # will use Campbooks. Each template is additive: it seeds extra tags and
  # document types on top of the built-in defaults and never removes anything
  # the user has already created.
  #
  # Templates are keyed by an identifier string stored in workspace.settings
  # under "setup_template". Existing workspaces without the key are unaffected
  # until they choose one from Settings → Setup.
  class Templates
    CATALOG = [
      {
        key: "freelancer",
        icon: "M20 7H4a2 2 0 00-2 2v9a2 2 0 002 2h16a2 2 0 002-2V9a2 2 0 00-2-2zM8 15H6v-2h2v2zm0-4H6V9h2v2zm4 4h-2v-2h2v2zm0-4h-2V9h2v2zm4 4h-2v-2h2v2zm0-4h-2V9h2v2zM6 5h12v2H6V5z",
        tags: [
          { name: "clients",      color: "#0584da", prompt: "Tag emails from or about clients, client projects, or customer work." },
          { name: "invoices",     color: "#00a8a8", prompt: "Tag emails that relate to invoices you issued or received." },
          { name: "projects",     color: "#7c3aed", prompt: "Tag emails tied to specific project work, deliverables, or milestones." }
        ],
        document_types: [
          { name: "invoice",     color: "#0584da", category: "accounting", prompt: "An invoice issued to a client for services or products. Extract: invoice number, date, amount, client name." },
          { name: "contract",    color: "#7c3aed", category: "legal",      prompt: "A contract or service agreement with a client or vendor. Extract: parties, date, value, term." },
          { name: "proposal",    color: "#00a8a8", category: "legal",      prompt: "A business proposal or quote sent to a prospective client. Extract: date, client, scope, price." },
          { name: "receipt",     color: "#d44996", category: "accounting", prompt: "A purchase receipt for a business expense. Extract: merchant, date, amount, items." }
        ],
        module_visibility: { "calendar" => true, "files" => true, "contacts" => true, "organizations" => true, "activity" => true }
      },
      {
        key: "small_business",
        icon: "M3 9l9-7 9 7v11a2 2 0 01-2 2H5a2 2 0 01-2-2z M9 22V12h6v10",
        tags: [
          { name: "suppliers",   color: "#d44996", prompt: "Tag emails from or about suppliers, vendors, or service providers." },
          { name: "team",        color: "#0584da", prompt: "Tag internal team emails, HR communications, or staff matters." },
          { name: "finance",     color: "#00a8a8", prompt: "Tag emails about financial matters: payments, taxes, banking, accounting." }
        ],
        document_types: [
          { name: "expense invoice",   color: "#0584da", category: "accounting", prompt: "A purchase or expense invoice received from a supplier. Extract: supplier, date, amount, VAT, invoice number." },
          { name: "revenue invoice",   color: "#00a8a8", category: "accounting", prompt: "An invoice issued to a customer for products or services. Extract: customer, date, amount, VAT, invoice number." },
          { name: "bank statement",    color: "#767988", category: "accounting", prompt: "A bank or credit-card statement. Extract: institution, period, opening balance, closing balance." },
          { name: "payslip",           color: "#7c3aed", category: "accounting", prompt: "An employee payslip or salary statement. Extract: employee name, period, gross pay, net pay, deductions." },
          { name: "contract",          color: "#d44996", category: "legal",      prompt: "A business contract or agreement. Extract: parties, effective date, value, term." }
        ],
        module_visibility: { "calendar" => true, "files" => true, "contacts" => true, "organizations" => true, "activity" => true }
      },
      {
        key: "personal_admin",
        icon: "M3 12l2-2m0 0l7-7 7 7M5 10v10a1 1 0 001 1h3m10-11l2 2m-2-2v10a1 1 0 01-1 1h-3m-6 0a1 1 0 001-1v-4a1 1 0 011-1h2a1 1 0 011 1v4a1 1 0 001 1m-6 0h6",
        tags: [
          { name: "bills",       color: "#0584da", prompt: "Tag emails about utility bills, rent, subscriptions, or household payments." },
          { name: "insurance",   color: "#00a8a8", prompt: "Tag emails from insurance companies: renewals, claims, policies." },
          { name: "government",  color: "#767988", prompt: "Tag emails from government agencies, tax authorities, or official bodies." }
        ],
        document_types: [
          { name: "insurance policy", color: "#00a8a8", category: "insurance",       prompt: "An insurance policy document. Extract: policy number, insurer, coverage type, premium, renewal date." },
          { name: "tax document",     color: "#767988", category: "accounting",       prompt: "A tax-related document: return, certificate, notice, or form. Extract: tax authority, year, amount, type." },
          { name: "bank statement",   color: "#0584da", category: "accounting",       prompt: "A bank or credit-card statement. Extract: institution, period, opening balance, closing balance." },
          { name: "receipt",          color: "#d44996", category: "accounting",       prompt: "A purchase receipt. Extract: merchant, date, amount, items." },
          { name: "identification",   color: "#7c3aed", category: "identification",   prompt: "An identity document: passport, ID card, driver licence. Extract: full name, document number, expiry." }
        ],
        module_visibility: { "calendar" => true, "files" => true, "contacts" => true, "organizations" => false, "activity" => false }
      },
      {
        key: "job_hunt",
        icon: "M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2m-6 9l2 2 4-4",
        tags: [
          { name: "applications", color: "#0584da", prompt: "Tag emails about job applications you have submitted." },
          { name: "interviews",   color: "#7c3aed", prompt: "Tag emails about interview invitations, confirmations, or follow-ups." },
          { name: "offers",       color: "#00a8a8", prompt: "Tag emails containing job offers, contracts, or offer negotiations." }
        ],
        document_types: [
          { name: "contract",        color: "#7c3aed", category: "legal",          prompt: "An employment contract or offer letter. Extract: employer, role, start date, salary, key terms." },
          { name: "correspondence",  color: "#767988", category: "correspondence",  prompt: "A formal letter or correspondence. Extract: sender, recipient, date, subject." }
        ],
        module_visibility: { "calendar" => true, "files" => true, "contacts" => true, "organizations" => false, "activity" => true }
      },
      {
        key: "just_exploring",
        icon: "M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z",
        tags: [],
        document_types: [],
        module_visibility: { "calendar" => true, "files" => true, "contacts" => true, "organizations" => true, "activity" => true }
      }
    ].freeze

    class << self
      # All templates as plain hashes.
      def all
        CATALOG
      end

      # Find a template by its key. Returns nil if not found.
      def find(key)
        CATALOG.find { |t| t[:key] == key.to_s }
      end

      # All valid template keys.
      def keys
        CATALOG.map { |t| t[:key] }
      end
    end
  end
end
