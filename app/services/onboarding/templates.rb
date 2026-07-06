# frozen_string_literal: true

module Onboarding
  # Persona-based setup templates that tailor a new workspace to how the user
  # will use Campbooks. Each template is additive: it seeds extra tags and
  # document types on top of the built-in defaults and never removes anything
  # the user has already created.
  #
  # Multiple templates can be combined (multi-select in onboarding). The
  # selected keys are stored as an array in workspace.settings["setup_templates"].
  #
  # Colors come from the sanctioned Campbooks::ColorDotSwatches::COLORS palette.
  class Templates
    # Valid document type categories (mirrors DocumentType::CATEGORIES).
    VALID_CATEGORIES = %w[accounting legal insurance identification correspondence other].freeze

    CATALOG = [
      {
        key: "freelancer",
        icon: "M20 7H4a2 2 0 00-2 2v9a2 2 0 002 2h16a2 2 0 002-2V9a2 2 0 00-2-2zM8 15H6v-2h2v2zm0-4H6V9h2v2zm4 4h-2v-2h2v2zm0-4h-2V9h2v2zm4 4h-2v-2h2v2zm0-4h-2V9h2v2zM6 5h12v2H6V5z",
        tags: [
          { name: "clients",  color: "#0584da", prompt: "Tag emails from or about clients, client projects, or customer work." },
          { name: "invoices", color: "#00a8a8", prompt: "Tag emails that relate to invoices you issued or received." },
          { name: "projects", color: "#595dec", prompt: "Tag emails tied to specific project work, deliverables, or milestones." }
        ],
        document_types: [
          { name: "invoice",  color: "#0584da", category: "accounting", prompt: "An invoice issued to a client for services or products. Extract: invoice number, date, amount, client name." },
          { name: "contract", color: "#595dec", category: "legal",      prompt: "A contract or service agreement with a client or vendor. Extract: parties, date, value, term." },
          { name: "proposal", color: "#00a8a8", category: "legal",      prompt: "A business proposal or quote sent to a prospective client. Extract: date, client, scope, price." },
          { name: "receipt",  color: "#d44996", category: "accounting", prompt: "A purchase receipt for a business expense. Extract: merchant, date, amount, items." }
        ],
        module_visibility: {
          "calendar" => true, "files" => true, "contacts" => true, "organizations" => true, "activity" => true
        }
      },
      {
        key: "small_business",
        icon: "M3 9l9-7 9 7v11a2 2 0 01-2 2H5a2 2 0 01-2-2z M9 22V12h6v10",
        tags: [
          { name: "suppliers", color: "#d44996", prompt: "Tag emails from or about suppliers, vendors, or service providers." },
          { name: "team",      color: "#0584da", prompt: "Tag internal team emails, HR communications, or staff matters." },
          { name: "finance",   color: "#00a8a8", prompt: "Tag emails about financial matters: payments, taxes, banking, accounting." }
        ],
        document_types: [
          { name: "expense invoice", color: "#0584da", category: "accounting", prompt: "A purchase or expense invoice received from a supplier. Extract: supplier, date, amount, VAT, invoice number." },
          { name: "revenue invoice", color: "#00a8a8", category: "accounting", prompt: "An invoice issued to a customer for products or services. Extract: customer, date, amount, VAT, invoice number." },
          { name: "bank statement",  color: "#767988", category: "accounting", prompt: "A bank or credit-card statement. Extract: institution, period, opening balance, closing balance." },
          { name: "payslip",         color: "#595dec", category: "accounting", prompt: "An employee payslip or salary statement. Extract: employee name, period, gross pay, net pay, deductions." },
          { name: "contract",        color: "#d44996", category: "legal",      prompt: "A business contract or agreement. Extract: parties, effective date, value, term." }
        ],
        module_visibility: {
          "calendar" => true, "files" => true, "contacts" => true, "organizations" => true, "activity" => true
        }
      },
      {
        key: "personal_admin",
        icon: "M3 12l2-2m0 0l7-7 7 7M5 10v10a1 1 0 001 1h3m10-11l2 2m-2-2v10a1 1 0 01-1 1h-3m-6 0a1 1 0 001-1v-4a1 1 0 011-1h2a1 1 0 011 1v4a1 1 0 001 1m-6 0h6",
        tags: [
          { name: "bills",      color: "#0584da", prompt: "Tag emails about utility bills, rent, subscriptions, or household payments." },
          { name: "insurance",  color: "#00a8a8", prompt: "Tag emails from insurance companies: renewals, claims, policies." },
          { name: "government", color: "#767988", prompt: "Tag emails from government agencies, tax authorities, or official bodies." }
        ],
        document_types: [
          { name: "insurance policy", color: "#00a8a8", category: "insurance",      prompt: "An insurance policy document. Extract: policy number, insurer, coverage type, premium, renewal date." },
          { name: "tax document",     color: "#767988", category: "accounting",      prompt: "A tax-related document: return, certificate, notice, or form. Extract: tax authority, year, amount, type." },
          { name: "bank statement",   color: "#0584da", category: "accounting",      prompt: "A bank or credit-card statement. Extract: institution, period, opening balance, closing balance." },
          { name: "receipt",          color: "#d44996", category: "accounting",      prompt: "A purchase receipt. Extract: merchant, date, amount, items." },
          { name: "identification",   color: "#595dec", category: "identification",  prompt: "An identity document: passport, ID card, driver licence. Extract: full name, document number, expiry." }
        ],
        module_visibility: {
          "calendar" => true, "files" => true, "contacts" => true, "organizations" => false, "activity" => false
        }
      },
      {
        key: "job_hunt",
        icon: "M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2m-6 9l2 2 4-4",
        tags: [
          { name: "applications", color: "#0584da", prompt: "Tag emails about job applications you have submitted." },
          { name: "interviews",   color: "#595dec", prompt: "Tag emails about interview invitations, confirmations, or follow-ups." },
          { name: "offers",       color: "#2ea55c", prompt: "Tag emails containing job offers, contracts, or offer negotiations." }
        ],
        document_types: [
          { name: "contract",       color: "#595dec", category: "legal",         prompt: "An employment contract or offer letter. Extract: employer, role, start date, salary, key terms." },
          { name: "correspondence", color: "#767988", category: "correspondence", prompt: "A formal letter or correspondence. Extract: sender, recipient, date, subject." }
        ],
        module_visibility: {
          "calendar" => true, "files" => true, "contacts" => true, "organizations" => false, "activity" => true
        }
      },
      {
        key: "accountant",
        icon: "M9 7h6m0 10v-3m-3 3h.01M9 17h.01M9 14h.01M12 14h.01M15 11h.01M12 11h.01M9 11h.01M7 21h10a2 2 0 002-2V5a2 2 0 00-2-2H7a2 2 0 00-2 2v14a2 2 0 002 2z",
        tags: [
          { name: "clients",   color: "#0584da", prompt: "Tag emails from or about accounting clients whose books or filings you manage." },
          { name: "deadlines", color: "#e76e08", prompt: "Tag emails that mention fiscal or tax deadlines, submission windows, or payment due dates." },
          { name: "fiscal",    color: "#00a8a8", prompt: "Tag emails related to fiscal, VAT, IVA, tax reporting, or regulatory compliance matters." }
        ],
        document_types: [
          { name: "e-invoice",     color: "#0584da", category: "accounting", prompt: "An electronic invoice (e-fatura, factura electronica) from a supplier or issued to a client. Extract: NIF/VAT, invoice number, date, total, tax amount, parties." },
          { name: "tax filing",    color: "#e76e08", category: "accounting", prompt: "A VAT/IVA return, income tax declaration, or other fiscal filing document. Extract: period, filing entity, total tax, reference number." },
          { name: "bank statement", color: "#767988", category: "accounting", prompt: "A bank or credit-card statement for reconciliation. Extract: institution, IBAN, period, opening balance, closing balance, transaction count." },
          { name: "payslip",       color: "#595dec", category: "accounting", prompt: "An employee payslip or salary statement. Extract: employee name, period, gross pay, net pay, deductions, social security." }
        ],
        module_visibility: {
          "calendar" => true, "files" => true, "contacts" => true, "organizations" => true, "activity" => true
        }
      },
      {
        key: "landlord",
        icon: "M8 14v3m4-3v3m4-3v3M3 21h18M3 10h18M3 7l9-4 9 4M4 10h16v11H4V10z",
        tags: [
          { name: "tenants",     color: "#0584da", prompt: "Tag emails from or about tenants, rental enquiries, or lease discussions." },
          { name: "properties",  color: "#2ea55c", prompt: "Tag emails relating to a specific property: viewings, maintenance, inspections, or rental listings." },
          { name: "maintenance", color: "#e76e08", prompt: "Tag emails about repairs, maintenance requests, contractor quotes, or building management issues." }
        ],
        document_types: [
          { name: "lease agreement",  color: "#0584da", category: "legal",      prompt: "A residential or commercial lease or tenancy agreement. Extract: parties, property address, rent amount, start/end dates, deposit." },
          { name: "rent receipt",     color: "#2ea55c", category: "accounting", prompt: "A rent payment receipt or rent invoice. Extract: tenant name, property, period, amount paid, payment date." },
          { name: "utility bill",     color: "#e76e08", category: "accounting", prompt: "A utility bill (electricity, water, gas, internet) for a rental property. Extract: provider, property address, period, amount, due date." },
          { name: "insurance policy", color: "#00a8a8", category: "insurance",  prompt: "A property or landlord insurance policy document. Extract: policy number, insurer, property, coverage, premium, renewal date." }
        ],
        module_visibility: {
          "calendar" => true, "files" => true, "contacts" => true, "organizations" => true, "activity" => true
        }
      },
      {
        key: "traveler",
        icon: "M3.055 11H5a2 2 0 012 2v1a2 2 0 002 2 2 2 0 012 2v2.945M8 3.935V5.5A2.5 2.5 0 0010.5 8h.5a2 2 0 012 2 2 2 0 104 0 2 2 0 012-2h1.064M15 20.488V18a2 2 0 012-2h3.064M21 12a9 9 0 11-18 0 9 9 0 0118 0z",
        tags: [
          { name: "travel",   color: "#0584da", prompt: "Tag emails related to upcoming trips, travel itineraries, or travel arrangements." },
          { name: "bookings", color: "#595dec", prompt: "Tag emails containing hotel, flight, car rental, or activity booking confirmations." },
          { name: "expenses", color: "#dca81c", prompt: "Tag emails about travel expenses, reimbursements, or cost tracking while travelling." }
        ],
        document_types: [
          { name: "booking confirmation", color: "#595dec", category: "correspondence", prompt: "A hotel, flight, or rental booking confirmation. Extract: booking reference, dates, destination, passenger or guest name, total cost." },
          { name: "itinerary",            color: "#0584da", category: "correspondence", prompt: "A travel itinerary document showing the sequence of flights, transfers, or activities. Extract: dates, destinations, carrier or operator, times." },
          { name: "boarding pass",        color: "#2ea55c", category: "identification", prompt: "An airline boarding pass (PDF or image). Extract: passenger name, flight number, origin, destination, date, seat, gate." },
          { name: "travel insurance",     color: "#00a8a8", category: "insurance",      prompt: "A travel insurance policy or certificate. Extract: policy number, insurer, insured name, trip dates, coverage region, emergency contact." }
        ],
        module_visibility: {
          "calendar" => true, "files" => true, "contacts" => true, "organizations" => false, "activity" => true
        }
      },
      {
        key: "ecommerce",
        icon: "M3 3h2l.4 2M7 13h10l4-8H5.4M7 13L5.4 5M7 13l-2.293 2.293c-.63.63-.184 1.707.707 1.707H17m0 0a2 2 0 100 4 2 2 0 000-4zm-8 2a2 2 0 11-4 0 2 2 0 014 0z",
        tags: [
          { name: "orders",    color: "#2ea55c", prompt: "Tag emails about customer orders: new orders, order confirmations, or order status updates." },
          { name: "suppliers", color: "#d44996", prompt: "Tag emails from or about product suppliers, manufacturers, or wholesalers." },
          { name: "returns",   color: "#e76e08", prompt: "Tag emails about product returns, refund requests, disputes, or chargebacks." },
          { name: "shipping",  color: "#0584da", prompt: "Tag emails from couriers, shipping carriers, or logistics providers about parcel tracking or delivery." }
        ],
        document_types: [
          { name: "purchase order",       color: "#2ea55c", category: "accounting",     prompt: "A purchase order sent to a supplier. Extract: PO number, supplier, items, quantities, unit prices, delivery date, total." },
          { name: "supplier invoice",     color: "#d44996", category: "accounting",     prompt: "An invoice received from a product supplier or manufacturer. Extract: invoice number, supplier, items, amounts, due date." },
          { name: "shipping document",    color: "#0584da", category: "correspondence", prompt: "A shipping label, delivery note, or customs document for a parcel. Extract: tracking number, carrier, origin, destination, contents, weight." },
          { name: "return authorization", color: "#e76e08", category: "correspondence", prompt: "A return merchandise authorization (RMA) or returns receipt. Extract: RMA number, customer, item returned, reason, refund amount." }
        ],
        module_visibility: {
          "calendar" => true, "files" => true, "contacts" => true, "organizations" => true, "activity" => true
        }
      },
      {
        key: "just_exploring",
        icon: "M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z",
        tags: [],
        document_types: [],
        module_visibility: {
          "calendar" => true, "files" => true, "contacts" => true, "organizations" => true, "activity" => true
        }
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
