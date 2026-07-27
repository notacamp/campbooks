# frozen_string_literal: true

require "rails_helper"

RSpec.describe Imap::HostGuard do
  subject(:guard) { described_class }

  # Ensure we are in cloud mode for most tests (not development, not self_hosted).
  before do
    allow(Rails).to receive(:env).and_return(ActiveSupport::StringInquirer.new("test"))
    allow(Rails.application.config).to receive(:self_hosted).and_return(false)
  end

  describe ".validate!" do
    context "when the host is blank" do
      it "raises BlockedError" do
        expect { guard.validate!("") }.to raise_error(Imap::HostGuard::BlockedError, /required/)
        expect { guard.validate!("  ") }.to raise_error(Imap::HostGuard::BlockedError)
        expect { guard.validate!(nil) }.to raise_error(Imap::HostGuard::BlockedError)
      end
    end

    context "when the host is a blocked hostname" do
      it "blocks localhost" do
        expect { guard.validate!("localhost") }.to raise_error(Imap::HostGuard::BlockedError)
      end

      it "blocks metadata.google.internal" do
        expect { guard.validate!("metadata.google.internal") }.to raise_error(Imap::HostGuard::BlockedError)
      end

      it "blocks metadata" do
        expect { guard.validate!("metadata") }.to raise_error(Imap::HostGuard::BlockedError)
      end

      it "blocks *.local hostnames" do
        expect { guard.validate!("mybox.local") }.to raise_error(Imap::HostGuard::BlockedError)
      end

      it "blocks *.internal hostnames" do
        expect { guard.validate!("mail.internal") }.to raise_error(Imap::HostGuard::BlockedError)
      end

      it "blocks *.localhost hostnames" do
        expect { guard.validate!("something.localhost") }.to raise_error(Imap::HostGuard::BlockedError)
      end

      it "strips a trailing FQDN dot before checking" do
        expect { guard.validate!("localhost.") }.to raise_error(Imap::HostGuard::BlockedError)
      end
    end

    context "when the host is a blocked literal IP" do
      it "blocks loopback IPv4" do
        expect { guard.validate!("127.0.0.1") }.to raise_error(Imap::HostGuard::BlockedError)
      end

      it "blocks loopback IPv6" do
        expect { guard.validate!("::1") }.to raise_error(Imap::HostGuard::BlockedError)
      end

      it "blocks bracketed IPv6 loopback (IMAP RFC 2822 host notation)" do
        expect { guard.validate!("[::1]") }.to raise_error(Imap::HostGuard::BlockedError)
      end

      it "blocks private IPv4 (10.x)" do
        expect { guard.validate!("10.0.0.1") }.to raise_error(Imap::HostGuard::BlockedError)
      end

      it "blocks private IPv4 (192.168.x)" do
        expect { guard.validate!("192.168.1.1") }.to raise_error(Imap::HostGuard::BlockedError)
      end

      it "blocks private IPv4 (172.16.x)" do
        expect { guard.validate!("172.16.0.1") }.to raise_error(Imap::HostGuard::BlockedError)
      end

      it "blocks link-local IPv4" do
        expect { guard.validate!("169.254.1.1") }.to raise_error(Imap::HostGuard::BlockedError)
      end

      it "blocks the GCP metadata IP (169.254.169.254)" do
        expect { guard.validate!("169.254.169.254") }.to raise_error(Imap::HostGuard::BlockedError)
      end

      it "blocks CGNAT (100.64.0.0/10)" do
        expect { guard.validate!("100.64.0.1") }.to raise_error(Imap::HostGuard::BlockedError)
        expect { guard.validate!("100.127.255.255") }.to raise_error(Imap::HostGuard::BlockedError)
      end

      it "blocks IPv4-mapped IPv6 of a loopback address" do
        expect { guard.validate!("::ffff:127.0.0.1") }.to raise_error(Imap::HostGuard::BlockedError)
      end

      it "blocks IPv4-mapped IPv6 of the metadata IP" do
        expect { guard.validate!("::ffff:169.254.169.254") }.to raise_error(Imap::HostGuard::BlockedError)
      end
    end

    context "when the host resolves to a blocked IP" do
      before do
        # Simulate a hostname that resolves to loopback — classic DNS rebinding
        # or split-horizon scenario.
        allow(Resolv).to receive(:getaddresses).and_return([ "127.0.0.1" ])
      end

      it "blocks the hostname" do
        expect { guard.validate!("mail.attacker.example") }.to raise_error(Imap::HostGuard::BlockedError)
      end
    end

    context "when DNS resolution fails" do
      before do
        allow(Resolv).to receive(:getaddresses).and_raise(Resolv::ResolvError)
      end

      it "allows the host (connect attempt will fail with a clearer error)" do
        expect { guard.validate!("nxdomain.invalid") }.not_to raise_error
      end
    end

    context "when DNS resolution times out" do
      before do
        allow(Resolv).to receive(:getaddresses).and_raise(Timeout::Error)
      end

      it "allows the host" do
        expect { guard.validate!("slow-resolving.example.com") }.not_to raise_error
      end
    end

    context "when the host is a legitimate public host" do
      before do
        allow(Resolv).to receive(:getaddresses).and_return([ "142.250.80.5" ])
      end

      it "allows a public hostname" do
        expect { guard.validate!("imap.gmail.com") }.not_to raise_error
      end
    end

    context "in development mode" do
      before do
        allow(Rails).to receive(:env).and_return(ActiveSupport::StringInquirer.new("development"))
      end

      it "allows localhost without raising" do
        expect { guard.validate!("localhost") }.not_to raise_error
      end

      it "allows private IPs without raising" do
        expect { guard.validate!("192.168.1.100") }.not_to raise_error
      end
    end

    context "when self_hosted is true" do
      before do
        allow(Rails.application.config).to receive(:self_hosted).and_return(true)
      end

      it "allows localhost without raising (self-hosters run their own mail server)" do
        expect { guard.validate!("localhost") }.not_to raise_error
      end

      it "allows private IPs without raising" do
        expect { guard.validate!("10.0.0.1") }.not_to raise_error
      end
    end
  end
end
